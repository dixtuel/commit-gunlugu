use axum::{
    body::Bytes,
    extract::State,
    http::HeaderMap,
    response::IntoResponse,
    Json,
};

use serde_json::{json, Value};
use uuid::Uuid;

use crate::crypto::hmac::verify_github_signature;
use crate::db::models::{Entry, Project, WebhookEvent};
use crate::db::{find_project_by_repo, insert_entry, log_webhook_event, upsert_project};
use crate::error::AppError;
use crate::sanitizer::sanitize_author;
use crate::state::AppState;


pub async fn handle_github_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    let event_type = headers
        .get("x-github-event")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let delivery_id = headers
        .get("x-github-delivery")
        .and_then(|h| h.to_str().ok())
        .unwrap_or_else(|| "none")
        .to_string();

    let sig_header = headers
        .get("x-hub-signature-256")
        .and_then(|h| h.to_str().ok());

    // 1. JSON payload ayrıştırma
    let payload: Value = serde_json::from_slice(&body)
        .map_err(|e| AppError::BadRequest(format!("Geçersiz JSON: {}", e)))?;

    let repo_full_name = payload
        .get("repository")
        .and_then(|r| r.get("full_name"))
        .and_then(|f| f.as_str())
        .unwrap_or("")
        .to_string();

    let repo_short_name = payload
        .get("repository")
        .and_then(|r| r.get("name"))
        .and_then(|f| f.as_str())
        .unwrap_or("Proje")
        .to_string();

    // 2. Proje tespiti ve Webhook secret doğrulama
    let project_opt = if !repo_full_name.is_empty() {
        find_project_by_repo(&state.db, &repo_full_name).await?
    } else {
        None
    };

    let decrypted_secret = project_opt
        .as_ref()
        .map(|p| crate::crypto::token::decrypt_token(&p.webhook_secret, state.config.token_encryption_key.as_deref()));

    let verification_result = if let Some(ref proj_sec) = decrypted_secret {
        if verify_github_signature(proj_sec, sig_header, &body).is_ok() {
            Ok(())
        } else {
            // Fallback: Eski/default webhook secret ile dene (geriye dönük uyumluluk)
            verify_github_signature(&state.config.default_webhook_secret, sig_header, &body)
        }
    } else {
        verify_github_signature(&state.config.default_webhook_secret, sig_header, &body)
    };

    verification_result?;

    tracing::info!(
        "Geçerli GitHub Webhook alındı: event={}, delivery={}, repo={}",
        event_type,
        delivery_id,
        repo_full_name
    );

    // Ping olayı hemen karşılanır
    if event_type == "ping" {
        return Ok(Json(json!({
            "status": "pong",
            "message": "GitHub Webhook bağlantısı başarıyla doğrulandı!",
            "delivery": delivery_id
        })));
    }

    // Proje veritabanında henüz yoksa otomatik oluştur (Zero-Config onboarding)
    let project = match project_opt {
        Some(p) => p,
        None => {
            if repo_full_name.is_empty() {
                return Ok(Json(json!({ "status": "ignored", "reason": "No repository in payload" })));
            }
            let slug = repo_full_name
                .replace('/', "-")
                .to_lowercase();
            let new_project = Project {
                id: Uuid::new_v4().to_string(),
                user_id: None,
                github_repo_full_name: repo_full_name.clone(),
                name: repo_short_name.clone(),
                slug,
                widget_key: format!("w_{}", Uuid::new_v4().simple()),
                brand_name: Some(repo_short_name),
                brand_color: "#10b981".to_string(),
                brand_logo_url: None,
                webhook_secret: state.config.default_webhook_secret.clone(),
                parse_mode: "ai_editorial".to_string(),
                audience: "end_user".to_string(),
                template_style: "standard".to_string(),
                is_private: 0,
                custom_github_token: None,
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            };
            upsert_project(&state.db, &new_project).await?;
            new_project
        }
    };

    // Webhook olayını kaydet
    let webhook_event = WebhookEvent {
        id: Uuid::new_v4().to_string(),
        project_id: Some(project.id.clone()),
        github_delivery_id: delivery_id.clone(),
        event_type: event_type.clone(),
        payload_summary: format!("repo: {}, sender: {:?}", repo_full_name, payload.get("sender").and_then(|s| s.get("login"))),
        processed_at: Some(chrono::Utc::now().to_rfc3339()),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let _ = log_webhook_event(&state.db, &webhook_event).await;

    // 3. Arka plan Tokio task'ı ile AI özetleme ve Entry oluşturma
    let state_clone = state.clone();
    let project_id = project.id.clone();

    tokio::spawn(async move {
        if let Err(e) = process_event_background(state_clone, project_id, event_type, payload).await {
            tracing::error!("Arka plan webhook işleme hatası: {:?}", e);
        }
    });

    Ok(Json(json!({
        "ok": true,
        "delivery": delivery_id,
        "project": project.slug
    })))
}

async fn process_event_background(
    state: AppState,
    project_id: String,
    event_type: String,
    payload: Value,
) -> Result<(), AppError> {
    match event_type.as_str() {
        "push" => {
            let commits = payload
                .get("commits")
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();

            if commits.is_empty() {
                return Ok(());
            }

            let mut commit_messages = Vec::new();
            let mut commit_shas = Vec::new();
            let mut primary_author = None;

            for c in commits {
                if let Some(msg) = c.get("message").and_then(|m| m.as_str()) {
                    let first_line = msg.lines().next().unwrap_or(msg);
                    commit_messages.push(first_line.to_string());
                }
                if let Some(sha) = c.get("id").and_then(|s| s.as_str()) {
                    commit_shas.push(sha.to_string());
                }
                if primary_author.is_none() {
                    let username = c.get("author").and_then(|a| a.get("username")).and_then(|u| u.as_str());
                    let name = c.get("author").and_then(|a| a.get("name")).and_then(|u| u.as_str());
                    primary_author = Some(sanitize_author(name, username));
                }
            }

            // Projenin parse_mode ayarını çek
            let project = crate::db::find_project_by_id(&state.db, &project_id).await?.unwrap_or_else(|| {
                crate::db::models::Project {
                    id: project_id.clone(),
                    user_id: None,
                    github_repo_full_name: "bilinmeyen/repo".to_string(),
                    name: "Proje".to_string(),
                    slug: "proje".to_string(),
                    widget_key: "w_def".to_string(),
                    brand_name: None,
                    brand_color: "#10b981".to_string(),
                    brand_logo_url: None,
                    webhook_secret: "".to_string(),
                    parse_mode: "ai_editorial".to_string(),
                    audience: "end_user".to_string(),
                    template_style: "standard".to_string(),
                    is_private: 0,
                    custom_github_token: None,
                    created_at: "".to_string(),
                    updated_at: "".to_string(),
                }
            });

            // Proje moduna göre ayrıştırma motorunu çalıştır
            let draft = match state.llm.summarize_for_project(
                &project.parse_mode,
                None,
                None,
                &commit_messages,
                &commit_shas,
                &[],
            ).await {
                Some(d) => d,
                None => return Ok(()),
            };

            let entry = Entry {
                id: Uuid::new_v4().to_string(),
                project_id,
                category: draft.category,
                title: draft.title,
                body: draft.body,
                status: "DRAFT".to_string(),
                ai_generated: if project.parse_mode == "ai_editorial" { 1 } else { 0 },
                source_commit_shas: serde_json::to_string(&commit_shas).unwrap_or_else(|_| "[]".to_string()),
                source_pr_number: None,
                author_username: primary_author,
                published_at: None,
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            };

            insert_entry(&state.db, &entry).await?;
            tracing::info!("Yeni push changelog taslağı eklendi (mod: {}): {}", project.parse_mode, entry.title);
        }
        "pull_request" => {
            let action = payload.get("action").and_then(|a| a.as_str()).unwrap_or("");
            let pr = payload.get("pull_request");
            let merged = pr
                .and_then(|p| p.get("merged"))
                .and_then(|m| m.as_bool())
                .unwrap_or(false);

            // Sadece birleştirilen (merged) PR'ları changelog'a dönüştür
            if action != "closed" || !merged {
                return Ok(());
            }

            let pr_title = pr.and_then(|p| p.get("title")).and_then(|t| t.as_str());
            let pr_body = pr.and_then(|p| p.get("body")).and_then(|b| b.as_str());
            let pr_number = pr.and_then(|p| p.get("number")).and_then(|n| n.as_i64());
            let author = pr
                .and_then(|p| p.get("user"))
                .and_then(|u| u.get("login"))
                .and_then(|l| l.as_str());

            let labels: Vec<String> = pr
                .and_then(|p| p.get("labels"))
                .and_then(|l| l.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.get("name").and_then(|n| n.as_str()))
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();

            let project = crate::db::find_project_by_id(&state.db, &project_id).await?.unwrap_or_else(|| {
                crate::db::models::Project {
                    id: project_id.clone(),
                    user_id: None,
                    github_repo_full_name: "bilinmeyen/repo".to_string(),
                    name: "Proje".to_string(),
                    slug: "proje".to_string(),
                    widget_key: "w_def".to_string(),
                    brand_name: None,
                    brand_color: "#10b981".to_string(),
                    brand_logo_url: None,
                    webhook_secret: "".to_string(),
                    parse_mode: "ai_editorial".to_string(),
                    audience: "end_user".to_string(),
                    template_style: "standard".to_string(),
                    is_private: 0,
                    custom_github_token: None,
                    created_at: "".to_string(),
                    updated_at: "".to_string(),
                }
            });

            let draft = match state.llm.summarize_for_project(
                &project.parse_mode,
                pr_title,
                pr_body,
                &[],
                &[],
                &labels,
            ).await {
                Some(d) => d,
                None => {
                    tracing::info!("PR etiketleri nedeniyle changelog atlandı (skip-changelog)");
                    return Ok(());
                }
            };

            let entry = Entry {
                id: Uuid::new_v4().to_string(),
                project_id,
                category: draft.category,
                title: draft.title,
                body: draft.body,
                status: "DRAFT".to_string(),
                ai_generated: 1,
                source_commit_shas: "[]".to_string(),
                source_pr_number: pr_number,
                author_username: Some(sanitize_author(None, author)),
                published_at: None,
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            };

            insert_entry(&state.db, &entry).await?;
            tracing::info!("Yeni PR changelog taslağı eklendi: {}", entry.title);
        }
        "commit_comment" => {
            let action = payload.get("action").and_then(|a| a.as_str()).unwrap_or("");
            if action != "created" {
                return Ok(());
            }

            let comment = payload.get("comment");
            let body = comment.and_then(|c| c.get("body")).and_then(|b| b.as_str()).unwrap_or("").trim();
            let commit_id = comment.and_then(|c| c.get("commit_id")).and_then(|id| id.as_str()).unwrap_or("");
            let author = comment.and_then(|c| c.get("user")).and_then(|u| u.get("login")).and_then(|l| l.as_str());

            if body.is_empty() {
                return Ok(());
            }

            let project = crate::db::find_project_by_id(&state.db, &project_id).await?.unwrap_or_else(|| {
                crate::db::models::Project {
                    id: project_id.clone(),
                    user_id: None,
                    github_repo_full_name: "bilinmeyen/repo".to_string(),
                    name: "Proje".to_string(),
                    slug: "proje".to_string(),
                    widget_key: "w_def".to_string(),
                    brand_name: None,
                    brand_color: "#10b981".to_string(),
                    brand_logo_url: None,
                    webhook_secret: "".to_string(),
                    parse_mode: "ai_editorial".to_string(),
                    audience: "end_user".to_string(),
                    template_style: "standard".to_string(),
                    is_private: 0,
                    custom_github_token: None,
                    created_at: "".to_string(),
                    updated_at: "".to_string(),
                }
            });

            let commit_shas = if !commit_id.is_empty() { vec![commit_id.to_string()] } else { vec![] };
            let commit_messages = vec![body.to_string()];

            let draft = match state.llm.summarize_for_project(
                &project.parse_mode,
                None,
                Some(body),
                &commit_messages,
                &commit_shas,
                &[],
            ).await {
                Some(d) => d,
                None => return Ok(()),
            };

            let entry = Entry {
                id: Uuid::new_v4().to_string(),
                project_id,
                category: draft.category,
                title: draft.title,
                body: draft.body,
                status: "DRAFT".to_string(),
                ai_generated: if project.parse_mode == "ai_editorial" { 1 } else { 0 },
                source_commit_shas: serde_json::to_string(&commit_shas).unwrap_or_else(|_| "[]".to_string()),
                source_pr_number: None,
                author_username: Some(sanitize_author(None, author)),
                published_at: None,
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            };

            insert_entry(&state.db, &entry).await?;
            let short_sha = if commit_id.len() >= 7 { &commit_id[..7] } else { commit_id };
            tracing::info!("Yeni commit_comment changelog taslağı eklendi (commit: {}): {}", short_sha, entry.title);
        }
        "release" => {
            let action = payload.get("action").and_then(|a| a.as_str()).unwrap_or("");
            if action != "published" {
                return Ok(());
            }

            let release = payload.get("release");
            let tag_name = release.and_then(|r| r.get("tag_name")).and_then(|t| t.as_str()).unwrap_or("");
            let name = release.and_then(|r| r.get("name")).and_then(|n| n.as_str()).unwrap_or(tag_name);
            let body = release.and_then(|r| r.get("body")).and_then(|b| b.as_str()).unwrap_or("").trim();
            let author = release.and_then(|r| r.get("author")).and_then(|u| u.get("login")).and_then(|l| l.as_str());

            let title = if !name.is_empty() {
                name.to_string()
            } else if !tag_name.is_empty() {
                format!("Sürüm {}", tag_name)
            } else {
                "Yeni Sürüm Yayında".to_string()
            };

            let entry = Entry {
                id: Uuid::new_v4().to_string(),
                project_id,
                category: "NEW".to_string(),
                title,
                body: if body.is_empty() { "Bu sürüm için detaylı not eklenmedi.".to_string() } else { body.to_string() },
                status: "PUBLISHED".to_string(), // Resmi GitHub Release yayınlandığı için doğrudan PUBLISHED olarak açılır
                ai_generated: 0,
                source_commit_shas: "[]".to_string(),
                source_pr_number: None,
                author_username: Some(sanitize_author(None, author)),
                published_at: Some(chrono::Utc::now().to_rfc3339()),
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            };

            insert_entry(&state.db, &entry).await?;
            tracing::info!("Resmi GitHub Release sürüm notu eklendi: {}", entry.title);
        }
        _ => {}
    }

    Ok(())
}
