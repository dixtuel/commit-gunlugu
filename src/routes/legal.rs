use axum::{
    extract::State,
    response::{Html, IntoResponse},
};
use minijinja::context;

use crate::error::AppError;
use crate::state::AppState;

pub async fn terms_page(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let tmpl = state
        .jinja
        .get_template("terms.html")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let html = tmpl
        .render(context! { app_url => state.config.app_url })
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Html(html))
}

pub async fn privacy_page(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let tmpl = state
        .jinja
        .get_template("privacy.html")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let html = tmpl
        .render(context! { app_url => state.config.app_url })
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Html(html))
}
