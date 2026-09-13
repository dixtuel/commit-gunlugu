use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse},
};
use minijinja::context;

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
