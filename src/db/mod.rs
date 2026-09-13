pub mod models;

use chrono::Utc;
use models::{Entry, PasswordReset, Project, User, WebhookEvent};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::str::FromStr;

use crate::error::AppError;

pub type DbPool = Pool<Sqlite>;

pub async fn init_db(database_url: &str) -> Result<DbPool, AppError> {
    let options = SqliteConnectOptions::from_str(database_url)
        .map_err(|e| AppError::Internal(format!("Geçersiz DATABASE_URL: {}", e)))?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .busy_timeout(std::time::Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(options)
        .await
        .map_err(AppError::Database)?;

    // Migrasyonları çalıştır
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| AppError::Internal(format!("Migrasyon hatası: {}", e)))?;

    tracing::info!("SQLite veritabanı başlatıldı (WAL modu aktif, Auth tabloları hazır)");
    Ok(pool)
}

// ---------------------------------------------------------------------------
// KULLANICI & KİMLİK DOĞRULAMA SORGULARI
// ---------------------------------------------------------------------------

pub async fn find_user_by_email(pool: &DbPool, email: &str) -> Result<Option<User>, AppError> {
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE LOWER(email) = LOWER(?)",
    )
    .bind(email.trim())
    .fetch_optional(pool)
    .await?;

    Ok(user)
}

#[allow(dead_code)]
pub async fn find_user_by_id(pool: &DbPool, id: &str) -> Result<Option<User>, AppError> {
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(user)
}

pub async fn create_user(pool: &DbPool, user: &User) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO users (id, email, password_hash, name, created_at, updated_at)
        VALUES (?, LOWER(?), ?, ?, datetime('now'), datetime('now'))
        "#,
    )
    .bind(&user.id)
    .bind(&user.email)
    .bind(&user.password_hash)
    .bind(&user.name)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_user_password(
    pool: &DbPool,
    user_id: &str,
    new_hash: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE users SET password_hash = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(new_hash)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// ŞİFRE SIFIRLAMA (PASSWORD RESET) SORGULARI
// ---------------------------------------------------------------------------

pub async fn create_password_reset(
    pool: &DbPool,
    id: &str,
    user_id: &str,
    token_hash: &str,
    expires_at: &str,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO password_resets (id, user_id, token_hash, expires_at, created_at)
        VALUES (?, ?, ?, ?, datetime('now'))
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn find_valid_password_reset(
    pool: &DbPool,
    token_hash: &str,
) -> Result<Option<PasswordReset>, AppError> {
    let now = Utc::now().to_rfc3339();
    let reset = sqlx::query_as::<_, PasswordReset>(
        r#"
        SELECT * FROM password_resets
        WHERE token_hash = ? AND used_at IS NULL AND expires_at > ?
        "#,
    )
    .bind(token_hash)
    .bind(now)
    .fetch_optional(pool)
    .await?;

    Ok(reset)
}

pub async fn mark_password_reset_used(pool: &DbPool, id: &str) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE password_resets SET used_at = datetime('now') WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// PROJE & İZOLASYON SORGULARI
// ---------------------------------------------------------------------------

pub async fn find_project_by_id(
    pool: &DbPool,
    id: &str,
) -> Result<Option<Project>, AppError> {
    let project = sqlx::query_as::<_, Project>(
        "SELECT * FROM projects WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(project)
}

pub async fn find_project_by_repo(
    pool: &DbPool,
    repo_full_name: &str,
) -> Result<Option<Project>, AppError> {
    let project = sqlx::query_as::<_, Project>(
        "SELECT * FROM projects WHERE LOWER(github_repo_full_name) = LOWER(?)",
    )
    .bind(repo_full_name)
    .fetch_optional(pool)
    .await?;

    Ok(project)
}

pub async fn find_project_by_slug(
    pool: &DbPool,
    slug: &str,
) -> Result<Option<Project>, AppError> {
    let project = sqlx::query_as::<_, Project>(
        "SELECT * FROM projects WHERE LOWER(slug) = LOWER(?)",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?;

    Ok(project)
}

pub async fn find_project_by_widget_key(
    pool: &DbPool,
    widget_key: &str,
) -> Result<Option<Project>, AppError> {
    let project = sqlx::query_as::<_, Project>(
        "SELECT * FROM projects WHERE widget_key = ?",
    )
    .bind(widget_key)
    .fetch_optional(pool)
    .await?;

    Ok(project)
}

/// Tüm projeleri listeler (sitemap ve public indeks için)
pub async fn list_all_projects(pool: &DbPool) -> Result<Vec<Project>, AppError> {
    let projects = sqlx::query_as::<_, Project>(
        "SELECT * FROM projects ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(projects)
}

/// Yalnızca belirli kullanıcıya ait projeleri listeler (Tenant İzolasyonu)
pub async fn list_projects_for_user(
    pool: &DbPool,
    user_id: &str,
) -> Result<Vec<Project>, AppError> {
    let projects = sqlx::query_as::<_, Project>(
        "SELECT * FROM projects WHERE user_id = ? ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(projects)
}

pub async fn upsert_project(
    pool: &DbPool,
    project: &Project,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO projects (id, user_id, github_repo_full_name, name, slug, widget_key, brand_name, brand_color, brand_logo_url, webhook_secret, parse_mode, audience, template_style, is_private, custom_github_token, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
        ON CONFLICT(github_repo_full_name) DO UPDATE SET
            user_id = COALESCE(excluded.user_id, projects.user_id),
            name = excluded.name,
            brand_name = excluded.brand_name,
            brand_color = excluded.brand_color,
            brand_logo_url = excluded.brand_logo_url,
            webhook_secret = excluded.webhook_secret,
            parse_mode = excluded.parse_mode,
            audience = excluded.audience,
            template_style = excluded.template_style,
            is_private = excluded.is_private,
            custom_github_token = COALESCE(excluded.custom_github_token, projects.custom_github_token),
            updated_at = datetime('now')
        "#
    )
    .bind(&project.id)
    .bind(&project.user_id)
    .bind(&project.github_repo_full_name)
    .bind(&project.name)
    .bind(&project.slug)
    .bind(&project.widget_key)
    .bind(&project.brand_name)
    .bind(&project.brand_color)
    .bind(&project.brand_logo_url)
    .bind(&project.webhook_secret)
    .bind(&project.parse_mode)
    .bind(&project.audience)
    .bind(&project.template_style)
    .bind(project.is_private)
    .bind(&project.custom_github_token)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn commit_sha_exists(pool: &DbPool, project_id: &str, sha: &str) -> Result<bool, AppError> {
    let pattern = format!("%{}%", sha);
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM entries WHERE project_id = ? AND source_commit_shas LIKE ? LIMIT 1"
    )
    .bind(project_id)
    .bind(pattern)
    .fetch_optional(pool)
    .await?;

    Ok(row.is_some())
}

pub async fn update_project_full_settings(
    pool: &DbPool,
    project_id: &str,
    user_id: &str,
    name: &str,
    brand_color: &str,
    parse_mode: &str,
    audience: &str,
    template_style: &str,
    is_private: i64,
    custom_github_token: Option<&str>,
) -> Result<bool, AppError> {
    let res = if let Some(token) = custom_github_token {
        sqlx::query(
            r#"
            UPDATE projects
            SET name = ?, brand_color = ?, parse_mode = ?, audience = ?, template_style = ?, is_private = ?, custom_github_token = ?, updated_at = datetime('now')
            WHERE id = ? AND user_id = ?
            "#
        )
        .bind(name)
        .bind(brand_color)
        .bind(parse_mode)
        .bind(audience)
        .bind(template_style)
        .bind(is_private)
        .bind(token)
        .bind(project_id)
        .bind(user_id)
        .execute(pool)
        .await?
    } else {
        sqlx::query(
            r#"
            UPDATE projects
            SET name = ?, brand_color = ?, parse_mode = ?, audience = ?, template_style = ?, is_private = ?, updated_at = datetime('now')
            WHERE id = ? AND user_id = ?
            "#
        )
        .bind(name)
        .bind(brand_color)
        .bind(parse_mode)
        .bind(audience)
        .bind(template_style)
        .bind(is_private)
        .bind(project_id)
        .bind(user_id)
        .execute(pool)
        .await?
    };

    Ok(res.rows_affected() > 0)
}

pub async fn delete_project(
    pool: &DbPool,
    project_id: &str,
    user_id: &str,
) -> Result<bool, AppError> {
    let res = sqlx::query(
        "DELETE FROM projects WHERE id = ? AND user_id = ?"
    )
    .bind(project_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(res.rows_affected() > 0)
}

pub async fn delete_entry(
    pool: &DbPool,
    entry_id: &str,
    user_id: &str,
) -> Result<bool, AppError> {
    let res = sqlx::query(
        r#"
        DELETE FROM entries
        WHERE id = ? AND project_id IN (SELECT id FROM projects WHERE user_id = ?)
        "#
    )
    .bind(entry_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(res.rows_affected() > 0)
}

// ---------------------------------------------------------------------------
// GİRİŞLER (ENTRIES) VE WEBHOOK LOGLARI
// ---------------------------------------------------------------------------

pub async fn insert_entry(pool: &DbPool, entry: &Entry) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO entries (
            id, project_id, category, title, body, status, ai_generated,
            source_commit_shas, source_pr_number, author_username, published_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#
    )
    .bind(&entry.id)
    .bind(&entry.project_id)
    .bind(&entry.category)
    .bind(&entry.title)
    .bind(&entry.body)
    .bind(&entry.status)
    .bind(entry.ai_generated)
    .bind(&entry.source_commit_shas)
    .bind(entry.source_pr_number)
    .bind(&entry.author_username)
    .bind(&entry.published_at)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_entries_for_project(
    pool: &DbPool,
    project_id: &str,
    published_only: bool,
) -> Result<Vec<Entry>, AppError> {
    let entries = if published_only {
        sqlx::query_as::<_, Entry>(
            "SELECT * FROM entries WHERE project_id = ? AND status = 'PUBLISHED' ORDER BY published_at DESC, created_at DESC LIMIT 50",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, Entry>(
            "SELECT * FROM entries WHERE project_id = ? ORDER BY created_at DESC LIMIT 100",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?
    };

    Ok(entries)
}

pub async fn list_entries_for_user(
    pool: &DbPool,
    user_id: &str,
    limit: i64,
) -> Result<Vec<Entry>, AppError> {
    let entries = sqlx::query_as::<_, Entry>(
        r#"
        SELECT e.* FROM entries e
        INNER JOIN projects p ON p.id = e.project_id
        WHERE p.user_id = ?
        ORDER BY e.created_at DESC
        LIMIT ?
        "#,
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(entries)
}

pub async fn list_all_recent_entries(pool: &DbPool, limit: i64) -> Result<Vec<Entry>, AppError> {
    let entries = sqlx::query_as::<_, Entry>(
        "SELECT * FROM entries ORDER BY created_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(entries)
}

pub async fn update_entry_status(
    pool: &DbPool,
    entry_id: &str,
    status: &str,
) -> Result<(), AppError> {
    let published_at = if status == "PUBLISHED" {
        Some(chrono::Utc::now().to_rfc3339())
    } else {
        None
    };

    sqlx::query(
        "UPDATE entries SET status = ?, published_at = COALESCE(?, published_at), updated_at = datetime('now') WHERE id = ?",
    )
    .bind(status)
    .bind(published_at)
    .bind(entry_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn log_webhook_event(
    pool: &DbPool,
    event: &WebhookEvent,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO webhook_events (id, project_id, github_delivery_id, event_type, payload_summary, processed_at)
        VALUES (?, ?, ?, ?, ?, datetime('now'))
        ON CONFLICT(github_delivery_id) DO NOTHING
        "#
    )
    .bind(&event.id)
    .bind(&event.project_id)
    .bind(&event.github_delivery_id)
    .bind(&event.event_type)
    .bind(&event.payload_summary)
    .execute(pool)
    .await?;

    Ok(())
}
