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
    let widget_key = if let Some(k) = query.key.filter(|k| !k.trim().is_empty()) {
        k
    } else if let Some(ref demo_key) = state.config.demo_widget_key {
        demo_key.clone()
    } else {
        sqlx::query_scalar::<_, String>("SELECT widget_key FROM projects ORDER BY created_at ASC LIMIT 1")
            .fetch_optional(&state.db)
            .await?
            .unwrap_or_else(|| "w_sample".to_string())
    };

    let tmpl = state
        .jinja
        .get_template("widget_preview.html")
        .map_err(|e| AppError::Internal(format!("Şablon yüklenemedi: {}", e)))?;

    let rendered = tmpl
        .render(context! {
            widget_key => widget_key,
            app_url => state.config.app_url,
        })
        .map_err(|e| AppError::Internal(format!("Şablon render hatası: {}", e)))?;

    Ok(Html(rendered))
}
