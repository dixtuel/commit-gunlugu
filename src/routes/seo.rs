use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
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
        Allow: /login\n\
        Allow: /register\n\
        Allow: /forgot-password\n\
        Allow: /reset-password\n\
        Allow: /widget-preview\n\
        Allow: /demo\n\
        Allow: /static/\n\
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

pub async fn google_verification(Path(token): Path<String>) -> Response {
    let token = token.strip_suffix(".html").unwrap_or(&token);
    if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return StatusCode::NOT_FOUND.into_response();
    }

    let path = format!("static/google{token}.html");
    serve_verification_file(&path, "text/html; charset=utf-8").await
}

pub async fn bing_verification() -> Response {
    serve_verification_file("static/BingSiteAuth.xml", "application/xml; charset=utf-8").await
}

pub async fn yandex_verification(Path(token): Path<String>) -> Response {
    let token = token.strip_suffix(".html").unwrap_or(&token);
    if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return StatusCode::NOT_FOUND.into_response();
    }

    let path = format!("static/yandex_{token}.html");
    serve_verification_file(&path, "text/html; charset=utf-8").await
}

async fn serve_verification_file(path: &str, content_type: &'static str) -> Response {
    match tokio::fs::read(path).await {
        Ok(body) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(content_type),
            );
            headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(CACHE_1WEEK));
            (StatusCode::OK, headers, body).into_response()
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            StatusCode::NOT_FOUND.into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
