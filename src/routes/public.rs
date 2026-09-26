use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse},
};
use minijinja::context;
use serde::Deserialize;

use crate::db::{find_project_by_slug, list_entries_for_project_branches};
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Deserialize, Default)]
pub struct PublicChangelogQuery {
    pub branch: Option<String>,
}

pub async fn public_changelog_page(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(query): Query<PublicChangelogQuery>,
) -> Result<impl IntoResponse, AppError> {
    let project = find_project_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("'{}' projesi bulunamadı", slug)))?;

    let filter_branches = if let Some(ref b) = query.branch.as_deref().map(str::trim).filter(|b| !b.is_empty()) {
        vec![b.to_string()]
    } else {
        project.tracked_branches()
    };

    let entries = list_entries_for_project_branches(
        &state.db,
        &project.id,
        true,
        &filter_branches,
        state.config.token_encryption_key.as_deref(),
    )
    .await?;

    let tmpl = state
        .jinja
        .get_template("changelog.html")
        .map_err(|e| AppError::Internal(format!("Şablon yüklenemedi: {}", e)))?;

    let rendered = tmpl
        .render(context! {
            project => project,
            entries => entries,
            app_url => state.config.app_url,
            current_branch => query.branch,
            tracked_branches => project.tracked_branches(),
        })
        .map_err(|e| AppError::Internal(format!("Şablon render hatası: {}", e)))?;

    Ok(Html(rendered))
}

#[derive(Debug, Deserialize, Default)]
pub struct WidgetPreviewQuery {
    pub key: Option<String>,
    pub branch: Option<String>,
}

/// Gömülebilir Widget Canlı Önizleme & Kod Üretici Stüdyosu
pub async fn widget_preview_page(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Query(query): Query<WidgetPreviewQuery>,
) -> Result<impl IntoResponse, AppError> {
    let current_user = if let Some(ref token) = crate::auth::session::extract_session_token(&headers) {
        crate::auth::session::get_user_from_session(&state.db, token, state.config.token_encryption_key.as_deref())
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    let (user_projects, is_authenticated) = if let Some(ref u) = current_user {
        let projs = crate::db::list_projects_for_user(&state.db, &u.id).await.unwrap_or_default();
        let mapped: Vec<serde_json::Value> = projs
            .into_iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.id,
                    "name": p.brand_name.clone().unwrap_or(p.name.clone()),
                    "slug": p.slug,
                    "widget_key": p.widget_key,
                    "brand_color": p.brand_color,
                    "tracked_branches": p.tracked_branches(),
                })
            })
            .collect();
        (mapped, true)
    } else {
        (Vec::new(), false)
    };

    let requested_key = query.key.as_deref().map(str::trim).filter(|k| !k.is_empty());
    let (preview_key, snippet_key, is_custom) = if let Some(k) = requested_key {
        (k.to_string(), k.to_string(), true)
    } else if let Some(first) = user_projects.first() {
        let k = first["widget_key"].as_str().unwrap_or("demo").to_string();
        (k.clone(), k, true)
    } else {
        ("demo".to_string(), "PROJE_WIDGET_ANAHTARINIZ".to_string(), false)
    };

    let tmpl = state
        .jinja
        .get_template("widget_preview.html")
        .map_err(|e| AppError::Internal(format!("Şablon yüklenemedi: {}", e)))?;

    let user_projects_json = serde_json::to_string(&user_projects).unwrap_or_else(|_| "[]".to_string());

    let rendered = tmpl
        .render(context! {
            preview_key => preview_key,
            snippet_key => snippet_key,
            is_custom => is_custom,
            app_url => state.config.app_url,
            is_authenticated => is_authenticated,
            current_user => current_user,
            user_projects => user_projects,
            user_projects_json => user_projects_json,
            initial_branch => query.branch.unwrap_or_default(),
        })
        .map_err(|e| AppError::Internal(format!("Şablon render hatası: {}", e)))?;

    Ok(Html(rendered))
}
