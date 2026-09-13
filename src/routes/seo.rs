use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
};

use crate::db::list_all_projects;
use crate::error::AppError;
use crate::state::AppState;

const CACHE_1DAY: &str = "public, max-age=86400, s-maxage=86400";
const CACHE_1WEEK: &str = "public, max-age=604800, s-maxage=604800";

pub async fn robots_txt(State(state): State<AppState>) -> impl IntoResponse {
    let base = state.config.app_url.trim_end_matches('/');
    let body = format!(
        "User-agent: *\n\
        Allow: /\n\
        Allow: /c/\n\
        Allow: /terms\n\
        Allow: /privacy\n\
        Allow: /static/\n\
        Disallow: /login\n\
        Disallow: /register\n\
        Disallow: /forgot-password\n\
        Disallow: /reset-password\n\
        Disallow: /dashboard\n\
        Disallow: /api/\n\n\
        Sitemap: {}/sitemap.xml\n",
        base
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(CACHE_1DAY));

    (headers, body)
}

pub async fn sitemap_xml(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let base = state.config.app_url.trim_end_matches('/');
    let projects = list_all_projects(&state.db).await?;

    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
"#,
    );

    // Ana sayfalar
    xml.push_str(&format!(
        "  <url><loc>{}/</loc><changefreq>daily</changefreq><priority>1.0</priority></url>\n",
        base
    ));
    xml.push_str(&format!(
        "  <url><loc>{}/terms</loc><changefreq>monthly</changefreq><priority>0.5</priority></url>\n",
        base
    ));
    xml.push_str(&format!(
        "  <url><loc>{}/privacy</loc><changefreq>monthly</changefreq><priority>0.5</priority></url>\n",
        base
    ));

    // Herkese açık proje sürüm günlükleri
    for p in projects {
        xml.push_str(&format!(
            "  <url><loc>{}/c/{}</loc><changefreq>weekly</changefreq><priority>0.8</priority></url>\n",
            base, p.slug
        ));
    }

    xml.push_str("</urlset>");

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/xml; charset=utf-8"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(CACHE_1DAY));

    Ok((headers, xml))
}

pub async fn google_verification(Path(token): Path<String>) -> impl IntoResponse {
    let body = format!("google-site-verification: google{}.html\n", token);
    let mut headers = HeaderMap::new();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(CACHE_1WEEK));
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    (StatusCode::OK, headers, body)
}
