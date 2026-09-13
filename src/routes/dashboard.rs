use axum::{
    extract::State,
    http::HeaderMap,
    response::{Html, IntoResponse, Redirect, Response},
};
use minijinja::context;

use crate::auth::session::{extract_session_token, get_user_from_session};
use crate::db::{list_all_projects, list_all_recent_entries, list_entries_for_user, list_projects_for_user};
use crate::error::AppError;
use crate::state::AppState;

pub async fn landing_page(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let current_user = if let Some(token) = extract_session_token(&headers) {
        get_user_from_session(&state.db, &token, state.config.token_encryption_key.as_deref()).await.ok().flatten()
    } else {
        None
    };

    let projects = list_all_projects(&state.db).await?;
    let entries = list_all_recent_entries(&state.db, 10).await?;

    let tmpl = state
        .jinja
        .get_template("index.html")
        .map_err(|e| AppError::Internal(format!("Şablon yüklenemedi: {}", e)))?;

    let rendered = tmpl
        .render(context! {
            current_user => current_user,
            projects => projects,
            recent_entries => entries,
            app_url => state.config.app_url,
        })
        .map_err(|e| AppError::Internal(format!("Render hatası: {}", e)))?;

    Ok(Html(rendered))
}

pub async fn dashboard_page(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    // 1. Kimlik Doğrulama Koruması (Auth Guard)
    let session_token = match extract_session_token(&headers) {
        Some(t) => t,
        None => return Ok(Redirect::to("/login?redirect=/dashboard").into_response()),
    };

    let user = match get_user_from_session(&state.db, &session_token, state.config.token_encryption_key.as_deref()).await? {
        Some(u) => u,
        None => return Ok(Redirect::to("/login?redirect=/dashboard").into_response()),
    };

    // 2. Tenant İzolasyonu: Yalnızca bu kullanıcıya ait projeler ve girişler
    let user_projects = list_projects_for_user(&state.db, &user.id).await?;
    let user_entries = list_entries_for_user(&state.db, &user.id, 50).await?;

    let tmpl = state
        .jinja
        .get_template("dashboard.html")
        .map_err(|e| AppError::Internal(format!("Şablon yüklenemedi: {}", e)))?;

    let rendered = tmpl
        .render(context! {
            current_user => user,
            projects => user_projects,
            entries => user_entries,
            app_url => state.config.app_url,
        })
        .map_err(|e| AppError::Internal(format!("Render hatası: {}", e)))?;

    Ok(Html(rendered).into_response())
}
