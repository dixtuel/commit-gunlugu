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
        .render(context! {
            app_url => state.config.app_url,
            legal_entity_name => state.config.legal_entity_name,
            privacy_contact_email => state.config.privacy_contact_email,
        })
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Html(html))
}

pub async fn privacy_page(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let tmpl = state
        .jinja
        .get_template("privacy.html")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let html = tmpl
        .render(context! {
            app_url => state.config.app_url,
            legal_entity_name => state.config.legal_entity_name,
            privacy_contact_email => state.config.privacy_contact_email,
        })
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Html(html))
}
