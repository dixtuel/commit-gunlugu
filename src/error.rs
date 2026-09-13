use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    Unauthorized(String),
    BadRequest(String),
    #[allow(dead_code)]
    RateLimited,
    Internal(String),
    Database(sqlx::Error),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(msg) => write!(f, "Bulunamadı: {}", msg),
            Self::Unauthorized(msg) => write!(f, "Yetkisiz erişim: {}", msg),
            Self::BadRequest(msg) => write!(f, "Geçersiz istek: {}", msg),
            Self::RateLimited => write!(f, "Çok fazla istek yapıldı, lütfen bekleyin"),
            Self::Internal(msg) => write!(f, "Sunucu hatası: {}", msg),
            Self::Database(err) => write!(f, "Veritabanı hatası: {}", err),
        }
    }
}

impl std::error::Error for AppError {}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        Self::Database(err)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        Self::BadRequest(err.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            Self::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            Self::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "Hız sınırı aşıldı (Rate limit exceeded)".to_string(),
            ),
            Self::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Dahili sunucu hatası".to_string())
            }
            Self::Database(err) => {
                tracing::error!("Database error: {:?}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı erişim hatası".to_string())
            }
        };

        let body = Json(json!({
            "error": true,
            "message": message,
            "code": status.as_u16(),
        }));

        (status, body).into_response()
    }
}
