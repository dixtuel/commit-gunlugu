use axum::{
    extract::Request,
    http::{header, HeaderName, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};

pub async fn security_headers(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    let mut response = next.run(req).await;
    let is_not_found = response.status() == StatusCode::NOT_FOUND;
    let headers = response.headers_mut();

    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::X_FRAME_OPTIONS,
        HeaderValue::from_static("SAMEORIGIN"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );

    if is_noindex_path(&path) || is_not_found {
        headers.insert(
            HeaderName::from_static("x-robots-tag"),
            HeaderValue::from_static("noindex, follow"),
        );
    }

    response
}

fn is_noindex_path(path: &str) -> bool {
    matches!(
        path,
        "/login"
            | "/register"
            | "/logout"
            | "/forgot-password"
            | "/reset-password"
            | "/dashboard"
            | "/terms"
            | "/privacy"
            | "/widget-preview"
            | "/demo"
    ) || (path.starts_with("/c/")
        && (path.ends_with("/export.md")
            || path.ends_with("/feed.xml")
            || path.ends_with("/feed.json")))
}
