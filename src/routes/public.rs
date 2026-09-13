use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse},
};
use minijinja::context;
use serde::Deserialize;

use crate::db::{find_project_by_slug, list_entries_for_project};
use crate::error::AppError;
use crate::state::AppState;

pub async fn public_changelog_page(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let project = find_project_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("'{}' projesi bulunamadı", slug)))?;

    let entries = list_entries_for_project(&state.db, &project.id, true).await?;

    let tmpl = state
        .jinja
        .get_template("changelog.html")
        .map_err(|e| AppError::Internal(format!("Şablon yüklenemedi: {}", e)))?;

    let rendered = tmpl
        .render(context! {
            project => project,
            entries => entries,
            app_url => state.config.app_url,
        })
        .map_err(|e| AppError::Internal(format!("Şablon render hatası: {}", e)))?;

    Ok(Html(rendered))
}

#[derive(Debug, Deserialize)]
pub struct WidgetPreviewQuery {
    pub key: Option<String>,
}

/// Gömülebilir Widget Canlı Önizleme & Kod Üretici Stüdyosu
pub async fn widget_preview_page(
    State(state): State<AppState>,
    Query(query): Query<WidgetPreviewQuery>,
) -> Result<impl IntoResponse, AppError> {
    let (preview_key, snippet_key, is_custom) = if let Some(k) = query.key.filter(|k| !k.trim().is_empty()) {
        (k.clone(), k, true)
    } else {
        ("demo".to_string(), "PROJE_WIDGET_ANAHTARINIZ".to_string(), false)
    };

    let tmpl = state
        .jinja
        .get_template("widget_preview.html")
        .map_err(|e| AppError::Internal(format!("Şablon yüklenemedi: {}", e)))?;

    let rendered = tmpl
        .render(context! {
            preview_key => preview_key,
            snippet_key => snippet_key,
            is_custom => is_custom,
            app_url => state.config.app_url,
        })
        .map_err(|e| AppError::Internal(format!("Şablon render hatası: {}", e)))?;

    Ok(Html(rendered))
}
