use axum::{
    extract::Request,
    http::{HeaderMap, HeaderValue},
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use crate::sanitizer::anonymize_ip;

/// Yüksek performanslı ve KVKK uyumlu temiz HTTP erişim loglayıcısı.
/// İstek süresini, metodunu, URL yolunu (query parametresiz), durum kodunu ve
/// subnet-maskeli anonim IP'yi kaydeder.
/// Her isteğe benzersiz `x-request-id` atar.
pub async fn http_logger(req: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path().to_string();

    let raw_ip = extract_client_ip(req.headers());
    let client_ip = anonymize_ip(&raw_ip);
    let req_id = uuid::Uuid::new_v4().simple().to_string()[..8].to_string();

    let response = next.run(req).await;
    let elapsed = start.elapsed();
    let status = response.status();

    // Statik varlıklar (CSS, JS, resim) için konsol kirliliğini önle
    if path.starts_with("/static/") {
        tracing::debug!(
            "[HTTP/STATIC] {} {} -> {} ({:.2?}) [id:{}]",
            method,
            path,
            status.as_u16(),
            elapsed,
            req_id
        );
    } else {
        match status.as_u16() {
            200..=299 => {
                tracing::info!(
                    "[HTTP] {} {} -> {} ({:.2?}) [ip: {}, id: {}]",
                    method,
                    path,
                    status,
                    elapsed,
                    client_ip,
                    req_id
                );
            }
            300..=399 => {
                let location = response
                    .headers()
                    .get(axum::http::header::LOCATION)
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("-");
                tracing::info!(
                    "[HTTP] {} {} -> {} (-> {}) ({:.2?}) [ip: {}, id: {}]",
                    method,
                    path,
                    status,
                    location,
                    elapsed,
                    client_ip,
                    req_id
                );
            }
            400..=499 => {
                tracing::warn!(
                    "[HTTP] {} {} -> {} ({:.2?}) [ip: {}, id: {}]",
                    method,
                    path,
                    status,
                    elapsed,
                    client_ip,
                    req_id
                );
            }
            _ => {
                tracing::error!(
                    "[HTTP] {} {} -> {} ({:.2?}) [ip: {}, id: {}]",
                    method,
                    path,
                    status,
                    elapsed,
                    client_ip,
                    req_id
                );
            }
        }
    }

    let mut response = response;
    if let Ok(val) = HeaderValue::from_str(&req_id) {
        response.headers_mut().insert("x-request-id", val);
    }

    response
}

fn extract_client_ip(headers: &HeaderMap) -> String {
    if let Some(cf_ip) = headers.get("cf-connecting-ip").and_then(|v| v.to_str().ok()) {
        return cf_ip.trim().to_string();
    }
    if let Some(x_forwarded) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = x_forwarded.split(',').next() {
            return first.trim().to_string();
        }
    }
    if let Some(real_ip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        return real_ip.trim().to_string();
    }
    "127.0.0.1".to_string()
}
