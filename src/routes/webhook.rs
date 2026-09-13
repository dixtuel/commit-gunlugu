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

    let secret = project_opt
        .as_ref()
        .map(|p| p.webhook_secret.as_str())
        .unwrap_or(state.config.default_webhook_secret.as_str());

    verify_github_signature(secret, sig_header, &body)?;

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

            // AI Fallback Motorunu çalıştır
            let draft = state.llm.summarize(None, None, &commit_messages).await;

            let entry = Entry {
                id: Uuid::new_v4().to_string(),
                project_id,
                category: draft.category,
                title: draft.title,
                body: draft.body,
                status: "DRAFT".to_string(),
                ai_generated: 1,
                source_commit_shas: serde_json::to_string(&commit_shas).unwrap_or_else(|_| "[]".to_string()),
                source_pr_number: None,
                author_username: primary_author,
                published_at: None,
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            };

            insert_entry(&state.db, &entry).await?;
            tracing::info!("Yeni push changelog taslağı eklendi: {}", entry.title);
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

            let draft = state.llm.summarize(pr_title, pr_body, &[]).await;

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
        _ => {}
    }

    Ok(())
}
