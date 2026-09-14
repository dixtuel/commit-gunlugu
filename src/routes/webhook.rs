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
use crate::db::{
    delete_entries_by_commit_shas, delete_entry_by_title_or_tag, find_project_by_repo,
    insert_entry, log_webhook_event, upsert_project,
};
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

/// GitHub Compare API kullanarak `after...before` karşılaştırması yapar.
/// Bu sayede force push veya branch geçmişi yeniden yazımında (rebase, squash, amend)
/// branch geçmişinden çıkarılan (ezilen / silinen) commit SHA'larını tam liste olarak döner.
async fn fetch_discarded_commits(
    repo: &str,
    after: &str,
    before: &str,
    token: Option<&str>,
) -> Vec<String> {
    if repo.is_empty()
        || after.is_empty()
        || before.is_empty()
        || after == before
        || after.chars().all(|c| c == '0')
        || before.chars().all(|c| c == '0')
    {
        return Vec::new();
    }

    let url = format!("https://api.github.com/repos/{}/compare/{}...{}", repo, after, before);

    let client = match reqwest::Client::builder()
        .user_agent("commit-gunlugu/0.1.0")
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("GitHub compare için reqwest client oluşturulamadı: {}", e);
            return Vec::new();
        }
    };

    let mut req = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");

    if let Some(t) = token {
        let trimmed = t.trim();
        if !trimmed.is_empty() {
            req = req.bearer_auth(trimmed);
        }
    }

    match req.send().await {
        Ok(res) => {
            if !res.status().is_success() {
                tracing::warn!(
                    "GitHub compare API yanıtı başarısız (status {}): repo={}, {}...{}",
                    res.status(),
                    repo,
                    after,
                    before
                );
                return Vec::new();
            }

            match res.json::<Value>().await {
                Ok(body) => {
                    let mut shas = Vec::new();
                    if let Some(commits) = body.get("commits").and_then(|c| c.as_array()) {
                        for c in commits {
                            if let Some(sha) = c.get("sha").and_then(|s| s.as_str()) {
                                shas.push(sha.to_string());
                            }
                        }
                    }
                    shas
                }
                Err(e) => {
                    tracing::warn!("GitHub compare API JSON parse hatası: {}", e);
                    Vec::new()
                }
            }
        }
        Err(e) => {
            tracing::warn!("GitHub compare API isteği başarısız: {}", e);
            Vec::new()
        }
    }
}

async fn process_event_background(
    state: AppState,
    project_id: String,
    event_type: String,
    payload: Value,
) -> Result<(), AppError> {
    match event_type.as_str() {
        "push" => {
            // Projenin ayarlarını ve repo bilgilerini çek
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

            let forced = payload.get("forced").and_then(|f| f.as_bool()).unwrap_or(false);
            let deleted = payload.get("deleted").and_then(|d| d.as_bool()).unwrap_or(false);
            let before = payload.get("before").and_then(|b| b.as_str()).unwrap_or("").trim();
            let after = payload.get("after").and_then(|a| a.as_str()).unwrap_or("").trim();

            let is_branch_deleted = deleted || after.chars().all(|c| c == '0') || after.is_empty();

            // 1. Force Push, Amend/Reset veya Branch Silinmesi Durumunda Eski Commit'leri Temizle
            if forced || is_branch_deleted {
                let repo_name = payload
                    .get("repository")
                    .and_then(|r| r.get("full_name"))
                    .and_then(|f| f.as_str())
                    .unwrap_or(&project.github_repo_full_name);

                let effective_token = project.custom_github_token.as_deref().and_then(|t| {
                    let decrypted = crate::crypto::token::decrypt_token(t.trim(), state.config.token_encryption_key.as_deref());
                    if decrypted.is_empty() { None } else { Some(decrypted) }
                }).or_else(|| state.config.github_token.clone());

                let mut discarded_shas = Vec::new();

                // Force push yapıldıysa ve her iki SHA da geçerliyse compare API ile geçmişten çıkarılan commit'leri bul
                if forced && !is_branch_deleted && !before.chars().all(|c| c == '0') && !before.is_empty() {
                    let mut api_shas = fetch_discarded_commits(repo_name, after, before, effective_token.as_deref()).await;
                    discarded_shas.append(&mut api_shas);
                }

                // Eski HEAD (before) geçerli bir SHA ise ve listede henüz yoksa ekle
                if !before.is_empty() && !before.chars().all(|c| c == '0') && !discarded_shas.iter().any(|s| s == before) {
                    discarded_shas.push(before.to_string());
                }

                if !discarded_shas.is_empty() {
                    match delete_entries_by_commit_shas(&state.db, &project_id, &discarded_shas).await {
                        Ok(deleted_titles) => {
                            if !deleted_titles.is_empty() {
                                tracing::info!(
                                    "Force push/silinme nedeniyle {} adet sürüm notu temizlendi (proje: {}, repo: {}): {:?}",
                                    deleted_titles.len(),
                                    project_id,
                                    repo_name,
                                    deleted_titles
                                );
                            }
                        }
                        Err(e) => {
                            tracing::error!("Force push sürüm notu temizleme hatası: {:?}", e);
                        }
                    }
                }
            }

            // Branch silindiyse ekleme yapmadan işlemi sonlandır
            if is_branch_deleted {
                return Ok(());
            }

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
                status: "PUBLISHED".to_string(),
                ai_generated: if project.parse_mode == "ai_editorial" { 1 } else { 0 },
                source_commit_shas: serde_json::to_string(&commit_shas).unwrap_or_else(|_| "[]".to_string()),
                source_pr_number: None,
                author_username: primary_author,
                published_at: Some(chrono::Utc::now().to_rfc3339()),
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            };

            insert_entry(&state.db, &entry).await?;
            tracing::info!("Yeni push sürüm notu otomatik yayına alındı (mod: {}): {}", project.parse_mode, entry.title);
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
                status: "PUBLISHED".to_string(),
                ai_generated: 1,
                source_commit_shas: "[]".to_string(),
                source_pr_number: pr_number,
                author_username: Some(sanitize_author(None, author)),
                published_at: Some(chrono::Utc::now().to_rfc3339()),
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            };

            insert_entry(&state.db, &entry).await?;
            tracing::info!("Yeni PR sürüm notu otomatik yayına alındı: {}", entry.title);
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
                status: "PUBLISHED".to_string(),
                ai_generated: if project.parse_mode == "ai_editorial" { 1 } else { 0 },
                source_commit_shas: serde_json::to_string(&commit_shas).unwrap_or_else(|_| "[]".to_string()),
                source_pr_number: None,
                author_username: Some(sanitize_author(None, author)),
                published_at: Some(chrono::Utc::now().to_rfc3339()),
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            };

            insert_entry(&state.db, &entry).await?;
            let short_sha = if commit_id.len() >= 7 { &commit_id[..7] } else { commit_id };
            tracing::info!("Yeni commit_comment sürüm notu otomatik yayına alındı (commit: {}): {}", short_sha, entry.title);
        }
        "release" => {
            let action = payload.get("action").and_then(|a| a.as_str()).unwrap_or("");
            if action == "deleted" {
                let release = payload.get("release");
                let tag_name = release.and_then(|r| r.get("tag_name")).and_then(|t| t.as_str()).unwrap_or("");
                let name = release.and_then(|r| r.get("name")).and_then(|n| n.as_str()).unwrap_or(tag_name);
                let title = if !name.is_empty() {
                    name.to_string()
                } else if !tag_name.is_empty() {
                    format!("Sürüm {}", tag_name)
                } else {
                    String::new()
                };

                if !title.is_empty() {
                    let _ = delete_entry_by_title_or_tag(&state.db, &project_id, &title).await;
                    tracing::info!("Silinen GitHub Release nedeniyle sürüm notu temizlendi: {}", title);
                }
                return Ok(());
            }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::db::init_db;
    use crate::llm::client::LlmFallbackEngine;

    fn mock_app_state(pool: crate::db::DbPool) -> AppState {
        let config = Config {
            host: "127.0.0.1".to_string(),
            port: 8095,
            database_url: "sqlite::memory:".to_string(),
            default_webhook_secret: "test_secret".to_string(),
            nvidia_nim_api_key: None,
            nvidia_nim_models: vec![],
            app_url: "http://localhost:8095".to_string(),
            github_token: None,
            webhook_rate_limit_per_minute: 60,
            api_rate_limit_per_minute: 60,
            token_encryption_key: None,
            smtp_host: None,
            smtp_port: 587,
            smtp_from: "noreply@example.com".to_string(),
            legal_entity_name: "Test Entity".to_string(),
            privacy_contact_email: None,
            demo_widget_key: None,
        };
        let llm = LlmFallbackEngine::new(config.clone());
        let jinja = minijinja::Environment::new();
        AppState::new(pool, llm, config, jinja)
    }

    #[tokio::test]
    async fn test_webhook_push_forced_deletes_old_entry() {
        let pool = init_db("sqlite::memory:", None).await.unwrap();
        let state = mock_app_state(pool.clone());
        let project_id = "test-proj-force-1";

        let project = Project {
            id: project_id.to_string(),
            user_id: None,
            github_repo_full_name: "owner/repo".to_string(),
            name: "Repo".to_string(),
            slug: "repo".to_string(),
            widget_key: "w_key".to_string(),
            brand_name: None,
            brand_color: "#10b981".to_string(),
            brand_logo_url: None,
            webhook_secret: "secret".to_string(),
            parse_mode: "ai_editorial".to_string(),
            audience: "end_user".to_string(),
            template_style: "standard".to_string(),
            is_private: 0,
            custom_github_token: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        upsert_project(&pool, &project).await.unwrap();

        let old_sha = "a1b2c3d4e5f67890abcdef1234567890abcdef12";
        let entry = Entry {
            id: "entry-to-be-deleted".to_string(),
            project_id: project_id.to_string(),
            category: "NEW".to_string(),
            title: "Eski Commit Başlığı".to_string(),
            body: "Açıklama".to_string(),
            status: "PUBLISHED".to_string(),
            ai_generated: 1,
            source_commit_shas: serde_json::to_string(&vec![old_sha]).unwrap(),
            source_pr_number: None,
            author_username: Some("dev".to_string()),
            published_at: Some(chrono::Utc::now().to_rfc3339()),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        insert_entry(&pool, &entry).await.unwrap();

        // Push payload: forced = true, before = old_sha (commit reset or amend)
        let payload = json!({
            "forced": true,
            "deleted": false,
            "before": old_sha,
            "after": "fedcba0987654321fedcba0987654321fedcba09",
            "commits": [],
            "repository": {
                "full_name": "owner/repo",
                "name": "repo"
            }
        });

        process_event_background(state, project_id.to_string(), "push".to_string(), payload)
            .await
            .unwrap();

        let remaining = crate::db::list_entries_for_project(&pool, project_id, false)
            .await
            .unwrap();
        assert_eq!(remaining.len(), 0, "Force push sonrası eski commit'e ait sürüm notu silinmelidir");
    }

    #[tokio::test]
    async fn test_webhook_push_deleted_branch_cleans_entry() {
        let pool = init_db("sqlite::memory:", None).await.unwrap();
        let state = mock_app_state(pool.clone());
        let project_id = "test-proj-del-1";

        let project = Project {
            id: project_id.to_string(),
            user_id: None,
            github_repo_full_name: "owner/del-repo".to_string(),
            name: "DelRepo".to_string(),
            slug: "del-repo".to_string(),
            widget_key: "w_del_key".to_string(),
            brand_name: None,
            brand_color: "#10b981".to_string(),
            brand_logo_url: None,
            webhook_secret: "secret".to_string(),
            parse_mode: "ai_editorial".to_string(),
            audience: "end_user".to_string(),
            template_style: "standard".to_string(),
            is_private: 0,
            custom_github_token: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        upsert_project(&pool, &project).await.unwrap();

        let branch_sha = "branch_head_sha_1234567890abcdef123456";
        let entry = Entry {
            id: "entry-branch-deleted".to_string(),
            project_id: project_id.to_string(),
            category: "NEW".to_string(),
            title: "Silinecek Dalın Notu".to_string(),
            body: "Açıklama".to_string(),
            status: "PUBLISHED".to_string(),
            ai_generated: 1,
            source_commit_shas: serde_json::to_string(&vec![branch_sha]).unwrap(),
            source_pr_number: None,
            author_username: Some("dev".to_string()),
            published_at: Some(chrono::Utc::now().to_rfc3339()),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        insert_entry(&pool, &entry).await.unwrap();

        let payload = json!({
            "forced": false,
            "deleted": true,
            "before": branch_sha,
            "after": "0000000000000000000000000000000000000000",
            "commits": [],
            "repository": {
                "full_name": "owner/del-repo",
                "name": "del-repo"
            }
        });

        process_event_background(state, project_id.to_string(), "push".to_string(), payload)
            .await
            .unwrap();

        let remaining = crate::db::list_entries_for_project(&pool, project_id, false)
            .await
            .unwrap();
        assert_eq!(remaining.len(), 0, "Silinen branch'e ait sürüm notu veritabanından temizlenmelidir");
    }

    #[tokio::test]
    async fn test_webhook_release_deleted_cleans_entry() {
        let pool = init_db("sqlite::memory:", None).await.unwrap();
        let state = mock_app_state(pool.clone());
        let project_id = "test-proj-rel-del";

        let project = Project {
            id: project_id.to_string(),
            user_id: None,
            github_repo_full_name: "owner/rel-repo".to_string(),
            name: "RelRepo".to_string(),
            slug: "rel-repo".to_string(),
            widget_key: "w_rel_key".to_string(),
            brand_name: None,
            brand_color: "#10b981".to_string(),
            brand_logo_url: None,
            webhook_secret: "secret".to_string(),
            parse_mode: "ai_editorial".to_string(),
            audience: "end_user".to_string(),
            template_style: "standard".to_string(),
            is_private: 0,
            custom_github_token: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        upsert_project(&pool, &project).await.unwrap();

        let entry = Entry {
            id: "entry-release-del".to_string(),
            project_id: project_id.to_string(),
            category: "NEW".to_string(),
            title: "v2.0.0 Büyük Sürüm".to_string(),
            body: "Açıklama".to_string(),
            status: "PUBLISHED".to_string(),
            ai_generated: 0,
            source_commit_shas: "[]".to_string(),
            source_pr_number: None,
            author_username: Some("dev".to_string()),
            published_at: Some(chrono::Utc::now().to_rfc3339()),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        insert_entry(&pool, &entry).await.unwrap();

        let payload = json!({
            "action": "deleted",
            "release": {
                "tag_name": "v2.0.0",
                "name": "v2.0.0 Büyük Sürüm"
            },
            "repository": {
                "full_name": "owner/rel-repo",
                "name": "rel-repo"
            }
        });

        process_event_background(state, project_id.to_string(), "release".to_string(), payload)
            .await
            .unwrap();

        let remaining = crate::db::list_entries_for_project(&pool, project_id, false)
            .await
            .unwrap();
        assert_eq!(remaining.len(), 0, "Silinen GitHub Release sürüm notu veritabanından temizlenmelidir");
    }
}
