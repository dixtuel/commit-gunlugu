use axum::http::HeaderMap;
use chrono::{Duration, Utc};
use sqlx::SqlitePool;

use crate::auth::password::{generate_session_token, hash_token};
use crate::crypto::token::decrypt_token;
use crate::db::models::User;
use crate::error::AppError;

pub const SESSION_COOKIE_NAME: &str = "cg_session";

/// HTTP HeaderMap içerisindeki Cookie başlığından `cg_session` değerini ayıklar.
pub fn extract_session_token(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for part in cookie_header.split(';') {
        let trimmed = part.trim();
        if let Some(val) = trimmed.strip_prefix("cg_session=") {
            let token = val.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}

/// Yeni bir oturum açar. Token veritabanında SHA-256 hash'lenmiş olarak saklanır (at-rest token hashing);
/// istemciye ise yalnızca ham çerez anahtarı teslim edilir.
pub async fn create_session(pool: &SqlitePool, user_id: &str) -> Result<String, AppError> {
    let raw_token = generate_session_token();
    let token_hash = hash_token(&raw_token);
    let expires_at = (Utc::now() + Duration::days(30)).to_rfc3339();

    sqlx::query(
        "INSERT INTO sessions (id, user_id, expires_at) VALUES (?, ?, ?)",
    )
    .bind(&token_hash)
    .bind(user_id)
    .bind(&expires_at)
    .execute(pool)
    .await?;

    Ok(raw_token)
}

/// Verilen oturum anahtarına karşılık gelen geçerli (süresi dolmamış) kullanıcıyı döndürür.
/// Hem yeni hash'lenmiş oturumları hem geriye dönük eski oturumları sabit zamanlı arar;
/// kullanıcının şifreli e-postasını çözer.
pub async fn get_user_from_session(
    pool: &SqlitePool,
    session_token: &str,
    key_hex: Option<&str>,
) -> Result<Option<User>, AppError> {
    let now = Utc::now().to_rfc3339();
    let token_hash = hash_token(session_token);

    let mut user = sqlx::query_as::<_, User>(
        r#"
        SELECT u.* FROM users u
        INNER JOIN sessions s ON s.user_id = u.id
        WHERE (s.id = ? OR s.id = ?) AND s.expires_at > ?
        "#,
    )
    .bind(&token_hash)
    .bind(session_token)
    .bind(now)
    .fetch_optional(pool)
    .await?;

    if let Some(ref mut u) = user {
        u.email = decrypt_token(&u.email, key_hex);
    }

    Ok(user)
}

/// Oturumu sonlandırır.
pub async fn destroy_session(pool: &SqlitePool, session_token: &str) -> Result<(), AppError> {
    let token_hash = hash_token(session_token);
    sqlx::query("DELETE FROM sessions WHERE id = ? OR id = ?")
        .bind(&token_hash)
        .bind(session_token)
        .execute(pool)
        .await?;

    Ok(())
}

/// Kullanıcının tüm açık oturumlarını sonlandırır (şifre sıfırlama vb. kritik anlarda).
pub async fn destroy_all_user_sessions(pool: &SqlitePool, user_id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub fn make_cookie_header(token: &str, app_url: &str) -> String {
    let secure = if app_url.starts_with("https://") { "; Secure" } else { "" };
    format!(
        "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000{}",
        SESSION_COOKIE_NAME, token, secure
    )
}

pub fn make_logout_cookie() -> String {
    format!(
        "{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
        SESSION_COOKIE_NAME
    )
}
