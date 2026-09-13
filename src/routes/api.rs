use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::session::{extract_session_token, get_user_from_session};
use crate::db::models::{Project, WidgetBrand, WidgetEntry, WidgetPayload};
use crate::db::{
    find_project_by_widget_key, list_all_projects, list_entries_for_project,
    list_projects_for_user, update_entry_status, upsert_project,
};
use crate::error::AppError;
use crate::state::AppState;

pub async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "service": "commit-gunlugu",
        "version": "0.1.0",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Gömülebilir JavaScript widget'ının tükettiği herkese açık JSON uç noktası.
/// CORS izinleri açıktır, rate limit ve gizlilik korumalıdır.
pub async fn get_widget_data(
    State(state): State<AppState>,
    Path(widget_key): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let project = find_project_by_widget_key(&state.db, &widget_key)
        .await?
        .ok_or_else(|| AppError::NotFound("Geçersiz widget anahtarı".to_string()))?;

    let entries = list_entries_for_project(&state.db, &project.id, true).await?;

    let widget_entries: Vec<WidgetEntry> = entries
        .into_iter()
        .map(|e| WidgetEntry {
            id: e.id,
            category: e.category,
            title: e.title,
            body: e.body,
            author: e.author_username,
            published_at: e.published_at.unwrap_or(e.created_at),
        })
        .collect();

    let changelog_url = format!("{}/c/{}", state.config.app_url.trim_end_matches('/'), project.slug);

    let payload = WidgetPayload {
        brand: WidgetBrand {
            name: project.brand_name.unwrap_or(project.name),
            color: project.brand_color,
            logo_url: project.brand_logo_url,
        },
        changelog_url,
        entries: widget_entries,
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=60, s-maxage=60"),
    );

    Ok((headers, Json(payload)))
}

pub async fn publish_entry_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(entry_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_session_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("Oturum açmanız gerekmektedir.".to_string()))?;
    get_user_from_session(&state.db, &token)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Geçersiz oturum.".to_string()))?;

    update_entry_status(&state.db, &entry_id, "PUBLISHED").await?;
    Ok(Json(json!({ "success": true, "status": "PUBLISHED" })))
}

pub async fn dismiss_entry_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(entry_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_session_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("Oturum açmanız gerekmektedir.".to_string()))?;
    get_user_from_session(&state.db, &token)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Geçersiz oturum.".to_string()))?;

    update_entry_status(&state.db, &entry_id, "DISMISSED").await?;
    Ok(Json(json!({ "success": true, "status": "DISMISSED" })))
}

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    pub github_repo_full_name: String,
    pub name: String,
    pub slug: Option<String>,
    pub brand_color: Option<String>,
}

pub async fn create_project_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateProjectRequest>,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_session_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("Proje eklemek için giriş yapmalısınız.".to_string()))?;
    let user = get_user_from_session(&state.db, &token)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Geçersiz oturum.".to_string()))?;

    let slug = payload.slug.unwrap_or_else(|| {
        payload.github_repo_full_name.replace('/', "-").to_lowercase()
    });

    let project = Project {
        id: Uuid::new_v4().to_string(),
        user_id: Some(user.id),
        github_repo_full_name: payload.github_repo_full_name,
        name: payload.name,
        slug,
        widget_key: format!("w_{}", Uuid::new_v4().simple()),
        brand_name: None,
        brand_color: payload.brand_color.unwrap_or_else(|| "#10b981".to_string()),
        brand_logo_url: None,
        webhook_secret: state.config.default_webhook_secret.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    upsert_project(&state.db, &project).await?;

    Ok((StatusCode::CREATED, Json(project)))
}

pub async fn list_projects_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_session_token(&headers);
    let user = if let Some(ref t) = token {
        get_user_from_session(&state.db, t).await?
    } else {
        None
    };

    let projects = if let Some(u) = user {
        list_projects_for_user(&state.db, &u.id).await?
    } else {
        list_all_projects(&state.db).await?
    };

    Ok(Json(projects))
}
