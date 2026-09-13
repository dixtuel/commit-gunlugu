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

pub async fn delete_entry_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(entry_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_session_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("Oturum açmanız gerekmektedir.".to_string()))?;
    let user = get_user_from_session(&state.db, &token)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Geçersiz oturum.".to_string()))?;

    let deleted = crate::db::delete_entry(&state.db, &entry_id, &user.id).await?;
    if deleted {
        Ok(Json(json!({ "success": true, "message": "Kayıt silindi" })))
    } else {
        Err(AppError::NotFound("Kayıt bulunamadı veya yetkiniz yok".to_string()))
    }
}

pub async fn delete_project_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_session_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("Oturum açmanız gerekmektedir.".to_string()))?;
    let user = get_user_from_session(&state.db, &token)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Geçersiz oturum.".to_string()))?;

    let deleted = crate::db::delete_project(&state.db, &project_id, &user.id).await?;
    if deleted {
        Ok(Json(json!({ "success": true, "message": "Proje silindi" })))
    } else {
        Err(AppError::NotFound("Proje bulunamadı veya yetkiniz yok".to_string()))
    }
}

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    pub github_repo_full_name: String,
    pub name: String,
    pub slug: Option<String>,
    pub brand_color: Option<String>,
    pub parse_mode: Option<String>,
    pub audience: Option<String>,
    pub template_style: Option<String>,
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
        brand_color: payload.brand_color.unwrap_or_else(|| "#2563eb".to_string()),
        brand_logo_url: None,
        webhook_secret: state.config.default_webhook_secret.clone(),
        parse_mode: payload.parse_mode.unwrap_or_else(|| "ai_editorial".to_string()),
        audience: payload.audience.unwrap_or_else(|| "end_user".to_string()),
        template_style: payload.template_style.unwrap_or_else(|| "standard".to_string()),
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    upsert_project(&state.db, &project).await?;

    Ok((StatusCode::CREATED, Json(project)))
}

#[derive(Deserialize)]
pub struct UpdateProjectSettingsRequest {
    pub name: Option<String>,
    pub brand_color: Option<String>,
    pub parse_mode: String,
    pub audience: String,
    pub template_style: String,
}

pub async fn update_project_settings_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Json(payload): Json<UpdateProjectSettingsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_session_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("Giriş yapmanız gerekmektedir.".to_string()))?;
    let user = get_user_from_session(&state.db, &token)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Geçersiz oturum.".to_string()))?;

    let existing = crate::db::find_project_by_id(&state.db, &project_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Proje bulunamadı".to_string()))?;

    if existing.user_id.as_deref() != Some(&user.id) {
        return Err(AppError::Unauthorized("Bu projeyi düzenleme yetkiniz yok".to_string()));
    }

    let name = payload.name.unwrap_or(existing.name);
    let brand_color = payload.brand_color.unwrap_or(existing.brand_color);

    crate::db::update_project_full_settings(
        &state.db,
        &project_id,
        &user.id,
        &name,
        &brand_color,
        &payload.parse_mode,
        &payload.audience,
        &payload.template_style,
    )
    .await?;

    Ok(Json(json!({ "success": true, "message": "Proje ayarları güncellendi" })))
}

#[derive(Deserialize)]
pub struct CreateManualEntryRequest {
    pub category: String,
    pub title: String,
    pub body: String,
    pub status: Option<String>,
}

pub async fn create_manual_entry_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Json(payload): Json<CreateManualEntryRequest>,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_session_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("Giriş yapmanız gerekmektedir.".to_string()))?;
    let user = get_user_from_session(&state.db, &token)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Geçersiz oturum.".to_string()))?;

    let project = crate::db::find_project_by_id(&state.db, &project_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Proje bulunamadı".to_string()))?;

    if project.user_id.as_deref() != Some(&user.id) {
        return Err(AppError::Unauthorized("Bu projeye kayıt ekleme yetkiniz yok".to_string()));
    }

    let status = payload.status.unwrap_or_else(|| "DRAFT".to_string());
    let published_at = if status == "PUBLISHED" {
        Some(chrono::Utc::now().to_rfc3339())
    } else {
        None
    };

    let author_username = user.name.clone().or_else(|| {
        Some(user.email.split('@').next().unwrap_or("user").to_string())
    });

    let entry = crate::db::models::Entry {
        id: Uuid::new_v4().to_string(),
        project_id,
        category: payload.category,
        title: payload.title,
        body: payload.body,
        status,
        ai_generated: 0,
        source_commit_shas: "[]".to_string(),
        source_pr_number: None,
        author_username,
        published_at,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    crate::db::insert_entry(&state.db, &entry).await?;

    Ok((StatusCode::CREATED, Json(entry)))
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
