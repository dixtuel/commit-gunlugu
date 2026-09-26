pub mod models;

use chrono::Utc;
use models::{Entry, PasswordReset, Project, User, WebhookEvent};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::collections::HashSet;
use std::str::FromStr;

use crate::auth::password::hash_email;
use crate::crypto::token::{
    decrypt_token, encrypt_token_for_storage, pack_text_for_storage, unpack_text_from_storage,
};
use crate::error::AppError;

pub type DbPool = Pool<Sqlite>;

pub async fn init_db(database_url: &str, token_encryption_key: Option<&str>) -> Result<DbPool, AppError> {
    let options = SqliteConnectOptions::from_str(database_url)
        .map_err(|e| AppError::Internal(format!("Geçersiz DATABASE_URL: {}", e)))?
        .create_if_missing(true)
        .foreign_keys(true)
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

    // Eski açık metin e-postaları güvenli şekilde şifreli ve hashli formata dönüştür (sıfır veri kaybı)
    if let Err(e) = migrate_encrypted_users(&pool, token_encryption_key).await {
        tracing::warn!("Eski kullanıcı e-postalarını şifreleme uyarısı: {}", e);
    }

    // Eski açık metin sürüm notu gövdelerini Zstandard (seviye 3, >= 512B) ve AES-256-GCM ile şeffaf dönüştür
    if let Err(e) = migrate_compressed_encrypted_entries(&pool, token_encryption_key).await {
        tracing::warn!("Eski sürüm notlarını sıkıştırma/şifreleme uyarısı: {}", e);
    }

    tracing::info!("SQLite veritabanı başlatıldı (WAL modu aktif, Auth ve Şifreleme tabloları hazır)");
    Ok(pool)
}

/// Henüz şifrelenmemiş veya email_hash'i üretilmemiş kullanıcıları otomatik olarak
/// AES-256-GCM ile şifreler ve deterministik kör indeks (email_hash) oluşturur.
pub async fn migrate_encrypted_users(pool: &DbPool, key_hex: Option<&str>) -> Result<(), AppError> {
    let users = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email_hash IS NULL OR email NOT LIKE 'enc:%'"
    )
    .fetch_all(pool)
    .await?;

    for u in users {
        let plain_email = decrypt_token(&u.email, key_hex);
        let e_hash = hash_email(&plain_email);
        let enc_email = encrypt_token_for_storage(&plain_email, key_hex);

        sqlx::query("UPDATE users SET email = ?, email_hash = ? WHERE id = ?")
            .bind(&enc_email)
            .bind(&e_hash)
            .bind(&u.id)
            .execute(pool)
            .await?;
    }

    Ok(())
}

/// Henüz şifrelenmemiş veya sıkıştırılmamış eski açık metin sürüm notu gövdelerini
/// Zstandard (seviye 3, >= 512B) ve AES-256-GCM ile şeffaf şekilde dönüştürür.
pub async fn migrate_compressed_encrypted_entries(pool: &DbPool, key_hex: Option<&str>) -> Result<(), AppError> {
    let Some(key_hex) = key_hex else {
        return Ok(());
    };

    let entries: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, body FROM entries WHERE body NOT LIKE 'enc:%'"
    )
    .fetch_all(pool)
    .await?;

    for (entry_id, raw_body) in entries {
        let plain_body = unpack_text_from_storage(&raw_body, Some(key_hex));
        let packed = pack_text_for_storage(&plain_body, Some(key_hex));

        sqlx::query("UPDATE entries SET body = ? WHERE id = ?")
            .bind(&packed)
            .bind(&entry_id)
            .execute(pool)
            .await?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// KULLANICI & KİMLİK DOĞRULAMA SORGULARI
// ---------------------------------------------------------------------------

pub async fn find_user_by_email(pool: &DbPool, email: &str, key_hex: Option<&str>) -> Result<Option<User>, AppError> {
    let email_trimmed = email.trim().to_lowercase();
    let email_hash = hash_email(&email_trimmed);

    let mut user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email_hash = ? OR LOWER(email) = LOWER(?)",
    )
    .bind(&email_hash)
    .bind(&email_trimmed)
    .fetch_optional(pool)
    .await?;

    if let Some(ref mut u) = user {
        u.email = decrypt_token(&u.email, key_hex);
    }

    Ok(user)
}

#[allow(dead_code)]
pub async fn find_user_by_id(pool: &DbPool, id: &str, key_hex: Option<&str>) -> Result<Option<User>, AppError> {
    let mut user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    if let Some(ref mut u) = user {
        u.email = decrypt_token(&u.email, key_hex);
    }

    Ok(user)
}

pub async fn create_user(pool: &DbPool, user: &User, key_hex: Option<&str>) -> Result<(), AppError> {
    let email_trimmed = user.email.trim().to_lowercase();
    let email_hash = hash_email(&email_trimmed);
    let stored_email = encrypt_token_for_storage(&email_trimmed, key_hex);

    sqlx::query(
        r#"
        INSERT INTO users (id, email, email_hash, password_hash, name, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, datetime('now'), datetime('now'))
        "#,
    )
    .bind(&user.id)
    .bind(&stored_email)
    .bind(&email_hash)
    .bind(&user.password_hash)
    .bind(&user.name)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_user_profile(
    pool: &DbPool,
    user_id: &str,
    name: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE users SET name = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(name)
    .bind(user_id)
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
        INSERT INTO projects (id, user_id, github_repo_full_name, name, slug, widget_key, brand_name, brand_color, brand_logo_url, webhook_secret, parse_mode, audience, template_style, language, tracked_branch, is_private, custom_github_token, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
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
            language = excluded.language,
            tracked_branch = excluded.tracked_branch,
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
    .bind(&project.language)
    .bind(&project.tracked_branch)
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
    language: &str,
    tracked_branch: &str,
    is_private: i64,
    custom_github_token: Option<&str>,
) -> Result<bool, AppError> {
    let res = if let Some(token) = custom_github_token {
        sqlx::query(
            r#"
            UPDATE projects
            SET name = ?, brand_color = ?, parse_mode = ?, audience = ?, template_style = ?, language = ?, tracked_branch = ?, is_private = ?, custom_github_token = ?, updated_at = datetime('now')
            WHERE id = ? AND user_id = ?
            "#
        )
        .bind(name)
        .bind(brand_color)
        .bind(parse_mode)
        .bind(audience)
        .bind(template_style)
        .bind(language)
        .bind(tracked_branch)
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
            SET name = ?, brand_color = ?, parse_mode = ?, audience = ?, template_style = ?, language = ?, tracked_branch = ?, is_private = ?, updated_at = datetime('now')
            WHERE id = ? AND user_id = ?
            "#
        )
        .bind(name)
        .bind(brand_color)
        .bind(parse_mode)
        .bind(audience)
        .bind(template_style)
        .bind(language)
        .bind(tracked_branch)
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

/// Force push, amend veya branch silinmesi durumunda ezilen/silinen commit SHA'larına
/// ait sürüm notlarını veritabanından temizler.
pub async fn delete_entries_by_commit_shas(
    pool: &DbPool,
    project_id: &str,
    shas: &[String],
) -> Result<Vec<String>, AppError> {
    if shas.is_empty() {
        return Ok(Vec::new());
    }

    let mut deleted_titles = Vec::new();
    let mut deleted_ids = HashSet::new();

    for sha in shas {
        let clean_sha = sha.trim();
        if clean_sha.len() < 7 {
            continue;
        }

        let pattern = format!("%{}%", clean_sha);

        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT id, title, source_commit_shas FROM entries WHERE project_id = ? AND source_commit_shas LIKE ?"
        )
        .bind(project_id)
        .bind(&pattern)
        .fetch_all(pool)
        .await?;

        for (entry_id, title, raw_shas) in rows {
            if deleted_ids.contains(&entry_id) {
                continue;
            }

            let matches = if let Ok(parsed_shas) = serde_json::from_str::<Vec<String>>(&raw_shas) {
                parsed_shas.iter().any(|s| {
                    let s_clean = s.trim();
                    s_clean == clean_sha
                        || (clean_sha.len() >= 7 && s_clean.starts_with(clean_sha))
                        || (s_clean.len() >= 7 && clean_sha.starts_with(s_clean))
                })
            } else {
                raw_shas.contains(clean_sha)
            };

            if matches {
                sqlx::query("DELETE FROM entries WHERE id = ?")
                    .bind(&entry_id)
                    .execute(pool)
                    .await?;

                deleted_ids.insert(entry_id);
                deleted_titles.push(title);
            }
        }
    }

    Ok(deleted_titles)
}

/// Bir girdiyi kaynak branch'e bağlar; aynı SHA başka branch'te görülürse
/// tek changelog girdisi korunup ikinci ilişki eklenir.
pub async fn assign_entry_branch(
    pool: &DbPool,
    entry_id: &str,
    project_id: &str,
    branch_name: &str,
) -> Result<(), AppError> {
    if branch_name.trim().is_empty() {
        return Ok(());
    }
    sqlx::query("INSERT OR IGNORE INTO entry_branches (entry_id, project_id, branch_name) VALUES (?, ?, ?)")
        .bind(entry_id)
        .bind(project_id)
        .bind(branch_name)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn assign_branch_to_existing_commit(
    pool: &DbPool,
    project_id: &str,
    sha: &str,
    branch_name: &str,
) -> Result<bool, AppError> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, source_commit_shas FROM entries WHERE project_id = ? AND source_commit_shas LIKE ?"
    )
    .bind(project_id)
    .bind(format!("%{}%", sha))
    .fetch_all(pool)
    .await?;
    for (entry_id, raw_shas) in rows {
        let matches = serde_json::from_str::<Vec<String>>(&raw_shas)
            .map(|items| items.iter().any(|item| item == sha))
            .unwrap_or_else(|_| raw_shas.contains(sha));
        if matches {
            assign_entry_branch(pool, &entry_id, project_id, branch_name).await?;
            return Ok(true);
        }
    }
    Ok(false)
}

/// Branch kapsamındaki ezilmiş commit'lerin yalnız ilgili branch ilişkisini kaldırır.
/// Girdi başka branch'lerde de kullanılıyorsa içerik korunur.
pub async fn remove_entry_branches_by_commit_shas(
    pool: &DbPool,
    project_id: &str,
    branch_name: &str,
    shas: &[String],
) -> Result<Vec<String>, AppError> {
    let mut removed_titles = Vec::new();
    for sha in shas {
        let clean_sha = sha.trim();
        if clean_sha.len() < 7 { continue; }
        let pattern = format!("%{}%", clean_sha);
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT e.id, e.title, e.source_commit_shas FROM entries e JOIN entry_branches b ON b.entry_id = e.id WHERE e.project_id = ? AND b.branch_name = ? AND e.source_commit_shas LIKE ?"
        )
        .bind(project_id).bind(branch_name).bind(&pattern).fetch_all(pool).await?;
        for (entry_id, title, raw_shas) in rows {
            let matches = serde_json::from_str::<Vec<String>>(&raw_shas)
                .map(|items| items.iter().any(|s| s.trim() == clean_sha || s.trim().starts_with(clean_sha) || clean_sha.starts_with(s.trim())))
                .unwrap_or_else(|_| raw_shas.contains(clean_sha));
            if !matches { continue; }
            sqlx::query("DELETE FROM entry_branches WHERE entry_id = ? AND branch_name = ?")
                .bind(&entry_id).bind(branch_name).execute(pool).await?;
            let remaining: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM entry_branches WHERE entry_id = ? LIMIT 1")
                .bind(&entry_id).fetch_optional(pool).await?;
            if remaining.is_none() {
                sqlx::query("DELETE FROM entries WHERE id = ?").bind(&entry_id).execute(pool).await?;
                removed_titles.push(title);
            }
        }
    }
    Ok(removed_titles)
}

pub async fn remove_all_entries_from_branch(
    pool: &DbPool,
    project_id: &str,
    branch_name: &str,
) -> Result<Vec<String>, AppError> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT e.id, e.title FROM entries e JOIN entry_branches b ON b.entry_id = e.id WHERE e.project_id = ? AND b.branch_name = ?"
    )
    .bind(project_id).bind(branch_name).fetch_all(pool).await?;
    let mut removed_titles = Vec::new();
    for (entry_id, title) in rows {
        sqlx::query("DELETE FROM entry_branches WHERE entry_id = ? AND branch_name = ?")
            .bind(&entry_id).bind(branch_name).execute(pool).await?;
        let remains: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM entry_branches WHERE entry_id = ? LIMIT 1")
            .bind(&entry_id).fetch_optional(pool).await?;
        if remains.is_none() {
            sqlx::query("DELETE FROM entries WHERE id = ?").bind(&entry_id).execute(pool).await?;
            removed_titles.push(title);
        }
    }
    Ok(removed_titles)
}

/// Başlığa veya etiket adına göre sürüm notunu siler (ör. silinen GitHub Release temizliği)
pub async fn delete_entry_by_title_or_tag(
    pool: &DbPool,
    project_id: &str,
    title_or_tag: &str,
) -> Result<bool, AppError> {
    let clean = title_or_tag.trim();
    if clean.is_empty() {
        return Ok(false);
    }

    let pattern = format!("%{}%", clean);
    let res = sqlx::query(
        "DELETE FROM entries WHERE project_id = ? AND (title = ? OR title LIKE ?)"
    )
    .bind(project_id)
    .bind(clean)
    .bind(&pattern)
    .execute(pool)
    .await?;

    Ok(res.rows_affected() > 0)
}

pub async fn delete_entry_by_title_for_branch(
    pool: &DbPool,
    project_id: &str,
    branch_name: &str,
    title_or_tag: &str,
) -> Result<bool, AppError> {
    let clean = title_or_tag.trim();
    if clean.is_empty() || branch_name.trim().is_empty() { return Ok(false); }
    let pattern = format!("%{}%", clean);
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT e.id FROM entries e JOIN entry_branches b ON b.entry_id = e.id WHERE e.project_id = ? AND b.branch_name = ? AND (e.title = ? OR e.title LIKE ?)"
    )
    .bind(project_id).bind(branch_name).bind(clean).bind(&pattern).fetch_all(pool).await?;
    let mut changed = false;
    for (entry_id,) in rows {
        sqlx::query("DELETE FROM entry_branches WHERE entry_id = ? AND branch_name = ?")
            .bind(&entry_id).bind(branch_name).execute(pool).await?;
        let remains: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM entry_branches WHERE entry_id = ? LIMIT 1")
            .bind(&entry_id).fetch_optional(pool).await?;
        if remains.is_none() {
            sqlx::query("DELETE FROM entries WHERE id = ?").bind(&entry_id).execute(pool).await?;
        }
        changed = true;
    }
    Ok(changed)
}

// ---------------------------------------------------------------------------
// GİRİŞLER (ENTRIES) VE WEBHOOK LOGLARI
// ---------------------------------------------------------------------------

pub async fn insert_entry(
    pool: &DbPool,
    entry: &Entry,
    key_hex: Option<&str>,
) -> Result<(), AppError> {
    let stored_body = pack_text_for_storage(&entry.body, key_hex);

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
    .bind(&stored_body)
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
    key_hex: Option<&str>,
) -> Result<Vec<Entry>, AppError> {
    let mut entries = if published_only {
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

    for e in &mut entries {
        e.body = unpack_text_from_storage(&e.body, key_hex);
    }

    Ok(entries)
}

/// Seçili branch'lere bağlı yayınları listeler. Boş seçim legacy davranışındaki gibi tüm girdileri döndürür.
pub async fn list_entries_for_project_branches(
    pool: &DbPool,
    project_id: &str,
    published_only: bool,
    branches: &[String],
    key_hex: Option<&str>,
) -> Result<Vec<Entry>, AppError> {
    let mut entries = if branches.is_empty() {
        list_entries_for_project(pool, project_id, published_only, key_hex).await?
    } else {
        let placeholders = vec!["?"; branches.len()].join(",");
        let status = if published_only { " AND e.status = 'PUBLISHED'" } else { "" };
        let query = format!(
            "SELECT DISTINCT e.* FROM entries e JOIN entry_branches b ON b.entry_id = e.id WHERE e.project_id = ? AND b.branch_name IN ({}){} ORDER BY e.published_at DESC, e.created_at DESC {}",
            placeholders, status, if published_only { "LIMIT 50" } else { "LIMIT 100" }
        );
        let mut q = sqlx::query_as::<_, Entry>(&query).bind(project_id);
        for branch in branches { q = q.bind(branch); }
        let mut rows = q.fetch_all(pool).await?;
        for entry in &mut rows { entry.body = unpack_text_from_storage(&entry.body, key_hex); }
        rows
    };
    // The unfiltered helper already decrypts; filtered rows are decrypted above.
    let _ = &mut entries;
    Ok(entries)
}

pub async fn list_entries_for_user(
    pool: &DbPool,
    user_id: &str,
    limit: i64,
    key_hex: Option<&str>,
) -> Result<Vec<Entry>, AppError> {
    let mut entries = sqlx::query_as::<_, Entry>(
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

    for e in &mut entries {
        e.body = unpack_text_from_storage(&e.body, key_hex);
    }

    Ok(entries)
}

pub async fn list_all_recent_entries(
    pool: &DbPool,
    limit: i64,
    key_hex: Option<&str>,
) -> Result<Vec<Entry>, AppError> {
    let mut entries = sqlx::query_as::<_, Entry>(
        "SELECT * FROM entries ORDER BY created_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    for e in &mut entries {
        e.body = unpack_text_from_storage(&e.body, key_hex);
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::password::hash_token;
    use crate::auth::session::{create_session, get_user_from_session};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_encrypted_user_roundtrip_and_blind_index() {
        let key = hex::encode([42u8; 32]);
        let pool = init_db("sqlite::memory:", Some(&key)).await.unwrap();

        let raw_email = "GizliTestUser@Example.Com";
        let user_id = Uuid::new_v4().to_string();
        let user = User {
            id: user_id.clone(),
            email: raw_email.to_string(),
            email_hash: None,
            password_hash: "argon2_test_hash".to_string(),
            name: Some("Test User".to_string()),
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };

        // 1. Kullanıcıyı kaydet (şifreli ve hashli)
        create_user(&pool, &user, Some(&key)).await.unwrap();

        // 2. Doğrudan DB'den ham sütunları kontrol et
        let (stored_email, stored_hash): (String, Option<String>) = sqlx::query_as(
            "SELECT email, email_hash FROM users WHERE id = ?"
        )
        .bind(&user_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        // Ham e-posta kesinlikle düz metin olmamalı, enc: öneki taşımalıdır!
        assert!(stored_email.starts_with("enc:"), "Veritabanındaki e-posta şifrelenmiş olmalıdır");
        assert!(!stored_email.contains("GizliTestUser"), "Açık e-posta veritabanında görünmemelidir");
        assert!(stored_hash.is_some(), "email_hash kör indeksi doldurulmuş olmalıdır");

        // 3. Büyük/küçük harf farketmeksizin blind index ile ara
        let found = find_user_by_email(&pool, "gizlitestuser@example.com", Some(&key))
            .await
            .unwrap()
            .expect("Kullanıcı bulunmalıdır");

        assert_eq!(found.id, user_id);
        assert_eq!(found.email, "gizlitestuser@example.com");

        // 4. Olmayan e-posta sorgusu None dönmeli
        let missing = find_user_by_email(&pool, "olmayan@example.com", Some(&key))
            .await
            .unwrap();
        assert!(missing.is_none(), "Kayıtsız e-posta None dönmelidir");

        // 5. Oturum aç ve token'ın DB'de hashli saklandığını doğrula
        let raw_session_token = create_session(&pool, &user_id).await.unwrap();
        let token_hash = hash_token(&raw_session_token);

        let (stored_session_id,): (String,) = sqlx::query_as(
            "SELECT id FROM sessions WHERE user_id = ?"
        )
        .bind(&user_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(stored_session_id, token_hash, "Oturum anahtarı veritabanında SHA-256 ile hashli saklanmalıdır");
        assert_ne!(stored_session_id, raw_session_token, "Oturum anahtarı düz metin saklanamaz");

        // 6. Ham session token ile kullanıcı oturumu çözümlenebilmelidir
        let session_user = get_user_from_session(&pool, &raw_session_token, Some(&key))
            .await
            .unwrap()
            .expect("Oturum geçerli olmalıdır");
        assert_eq!(session_user.id, user_id);
        assert_eq!(session_user.email, "gizlitestuser@example.com");
    }

    #[tokio::test]
    async fn test_delete_entries_by_commit_shas() {
        let pool = init_db("sqlite::memory:", None).await.unwrap();
        let project_id = "test-proj-123";

        let project = Project {
            id: project_id.to_string(),
            user_id: None,
            github_repo_full_name: "test/repo".to_string(),
            name: "Test Repo".to_string(),
            slug: "test-repo".to_string(),
            widget_key: "w_test".to_string(),
            brand_name: None,
            brand_color: "#10b981".to_string(),
            brand_logo_url: None,
            webhook_secret: "secret".to_string(),
            parse_mode: "ai_editorial".to_string(),
            audience: "end_user".to_string(),
            template_style: "standard".to_string(),
            language: "auto".to_string(),
            tracked_branch: "".to_string(),
            is_private: 0,
            custom_github_token: None,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };
        upsert_project(&pool, &project).await.unwrap();

        let entry1 = Entry {
            id: "entry-1".to_string(),
            project_id: project_id.to_string(),
            category: "NEW".to_string(),
            title: "Eski Amend Edilen Özellik".to_string(),
            body: "Açıklama".to_string(),
            status: "PUBLISHED".to_string(),
            ai_generated: 1,
            source_commit_shas: serde_json::to_string(&vec!["old_sha_1234567890abcdef"]).unwrap(),
            source_pr_number: None,
            author_username: Some("dev".to_string()),
            published_at: Some(Utc::now().to_rfc3339()),
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };

        let entry2 = Entry {
            id: "entry-2".to_string(),
            project_id: project_id.to_string(),
            category: "FIX".to_string(),
            title: "Kalan Kalıcı Düzeltme".to_string(),
            body: "Açıklama".to_string(),
            status: "PUBLISHED".to_string(),
            ai_generated: 1,
            source_commit_shas: serde_json::to_string(&vec!["keep_sha_9876543210fedcba"]).unwrap(),
            source_pr_number: None,
            author_username: Some("dev".to_string()),
            published_at: Some(Utc::now().to_rfc3339()),
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };

        insert_entry(&pool, &entry1, None).await.unwrap();
        insert_entry(&pool, &entry2, None).await.unwrap();

        // 1. Eski sha silindiğinde (force push / amend simülasyonu)
        let deleted = delete_entries_by_commit_shas(
            &pool,
            project_id,
            &["old_sha_1234567890abcdef".to_string()],
        )
        .await
        .unwrap();

        assert_eq!(deleted.len(), 1);
        assert_eq!(deleted[0], "Eski Amend Edilen Özellik");

        // 2. Kalan entry kontrolü
        let remaining = list_entries_for_project(&pool, project_id, false, None)
            .await
            .unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "entry-2");

        // 3. Olmayan veya çok kısa sha çağrısı hiçbir şeyi silmemeli
        let deleted_empty = delete_entries_by_commit_shas(
            &pool,
            project_id,
            &["short".to_string()],
        )
        .await
        .unwrap();
        assert_eq!(deleted_empty.len(), 0);

        let remaining_after = list_entries_for_project(&pool, project_id, false, None)
            .await
            .unwrap();
        assert_eq!(remaining_after.len(), 1);
    }

    #[tokio::test]
    async fn test_insert_and_list_entries_with_compression_and_encryption() {
        let key = hex::encode([42u8; 32]);
        let pool = init_db("sqlite::memory:", Some(&key)).await.unwrap();
        let project_id = "test-proj-compression";

        let project = Project {
            id: project_id.to_string(),
            user_id: None,
            github_repo_full_name: "test/compression".to_string(),
            name: "Compression Test".to_string(),
            slug: "compression-test".to_string(),
            widget_key: "w_comp".to_string(),
            brand_name: None,
            brand_color: "#10b981".to_string(),
            brand_logo_url: None,
            webhook_secret: "secret".to_string(),
            parse_mode: "ai_editorial".to_string(),
            audience: "end_user".to_string(),
            template_style: "standard".to_string(),
            language: "auto".to_string(),
            tracked_branch: "".to_string(),
            is_private: 0,
            custom_github_token: None,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };
        upsert_project(&pool, &project).await.unwrap();

        let long_body = "Sistem üzerinde yapılan bu sürüm ile veritabanı şifreleme ve Zstandard sıkıştırma mimarisi kuruldu. ".repeat(15);
        assert!(long_body.len() > 1000);

        let entry = Entry {
            id: "entry-comp-1".to_string(),
            project_id: project_id.to_string(),
            category: "NEW".to_string(),
            title: "Büyük Sıkıştırılmış Sürüm".to_string(),
            body: long_body.clone(),
            status: "PUBLISHED".to_string(),
            ai_generated: 1,
            source_commit_shas: "[]".to_string(),
            source_pr_number: None,
            author_username: Some("dev".to_string()),
            published_at: Some(Utc::now().to_rfc3339()),
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };

        // 1. Veriyi şifreli ve Zstd sıkıştırmalı kaydet
        insert_entry(&pool, &entry, Some(&key)).await.unwrap();

        // 2. Doğrudan SQLite'tan ham veriyi kontrol et: enc:zstd: ile başlamalı!
        let (raw_db_body,): (String,) = sqlx::query_as(
            "SELECT body FROM entries WHERE id = ?"
        )
        .bind(&entry.id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert!(raw_db_body.starts_with("enc:zstd:"), "Ham veritabanı sütununda enc:zstd: öneki bulunmalıdır");
        assert!(!raw_db_body.contains("Sistem üzerinde yapılan"), "Açık metin veritabanında görünmemelidir");

        // 3. list_entries_for_project ile şeffaf okuma
        let fetched = list_entries_for_project(&pool, project_id, true, Some(&key)).await.unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].body, long_body, "Okunan veri eksiksiz ve hatasız decompress edilmiş olmalıdır");
    }

    #[tokio::test]
    async fn test_multi_branch_entry_assignment_and_filtering() {
        let pool = init_db("sqlite::memory:", None).await.unwrap();
        let project_id = "test-proj-branches";

        let project = Project {
            id: project_id.to_string(),
            user_id: None,
            github_repo_full_name: "test/multibranch".to_string(),
            name: "MultiBranch Test".to_string(),
            slug: "multibranch-test".to_string(),
            widget_key: "w_mb".to_string(),
            brand_name: None,
            brand_color: "#10b981".to_string(),
            brand_logo_url: None,
            webhook_secret: "secret".to_string(),
            parse_mode: "ai_editorial".to_string(),
            audience: "end_user".to_string(),
            template_style: "standard".to_string(),
            language: "auto".to_string(),
            tracked_branch: "main\nbeta".to_string(),
            is_private: 0,
            custom_github_token: None,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };
        upsert_project(&pool, &project).await.unwrap();

        let entry_main = Entry {
            id: "entry-main".to_string(),
            project_id: project_id.to_string(),
            category: "NEW".to_string(),
            title: "Main Branch Feature".to_string(),
            body: "Main desc".to_string(),
            status: "PUBLISHED".to_string(),
            ai_generated: 0,
            source_commit_shas: "[]".to_string(),
            source_pr_number: None,
            author_username: Some("dev".to_string()),
            published_at: Some(Utc::now().to_rfc3339()),
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };

        let entry_beta = Entry {
            id: "entry-beta".to_string(),
            project_id: project_id.to_string(),
            category: "IMPROVEMENT".to_string(),
            title: "Beta Branch Improvement".to_string(),
            body: "Beta desc".to_string(),
            status: "PUBLISHED".to_string(),
            ai_generated: 0,
            source_commit_shas: "[]".to_string(),
            source_pr_number: None,
            author_username: Some("dev".to_string()),
            published_at: Some(Utc::now().to_rfc3339()),
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };

        let entry_dev = Entry {
            id: "entry-dev".to_string(),
            project_id: project_id.to_string(),
            category: "FIX".to_string(),
            title: "Dev Untracked Fix".to_string(),
            body: "Dev desc".to_string(),
            status: "PUBLISHED".to_string(),
            ai_generated: 0,
            source_commit_shas: "[]".to_string(),
            source_pr_number: None,
            author_username: Some("dev".to_string()),
            published_at: Some(Utc::now().to_rfc3339()),
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };

        insert_entry(&pool, &entry_main, None).await.unwrap();
        insert_entry(&pool, &entry_beta, None).await.unwrap();
        insert_entry(&pool, &entry_dev, None).await.unwrap();

        assign_entry_branch(&pool, &entry_main.id, project_id, "main").await.unwrap();
        assign_entry_branch(&pool, &entry_beta.id, project_id, "beta").await.unwrap();
        assign_entry_branch(&pool, &entry_dev.id, project_id, "dev").await.unwrap();

        // 1. main ve beta filtrelemesi (projenin tracked branches'i)
        let tracked = list_entries_for_project_branches(
            &pool,
            project_id,
            true,
            &["main".to_string(), "beta".to_string()],
            None,
        )
        .await
        .unwrap();
        assert_eq!(tracked.len(), 2);
        assert!(tracked.iter().any(|e| e.id == "entry-main"));
        assert!(tracked.iter().any(|e| e.id == "entry-beta"));
        assert!(!tracked.iter().any(|e| e.id == "entry-dev"));

        // 2. Yalnızca beta filtrelemesi
        let beta_only = list_entries_for_project_branches(
            &pool,
            project_id,
            true,
            &["beta".to_string()],
            None,
        )
        .await
        .unwrap();
        assert_eq!(beta_only.len(), 1);
        assert_eq!(beta_only[0].id, "entry-beta");

        // 3. Boş filtre (tüm branch'ler)
        let all = list_entries_for_project_branches(&pool, project_id, true, &[], None)
            .await
            .unwrap();
        assert_eq!(all.len(), 3);
    }
}
