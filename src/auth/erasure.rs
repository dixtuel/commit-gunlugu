use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use uuid::Uuid;

use crate::error::AppError;

pub const LOCAL_LEDGER_PATH: &str = "data/erasure-ledger.jsonl";

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct ErasureRecord {
    pub user_id: String,
    pub email_hash: String,
    pub purged_at: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct RetentionReport {
    pub purged_ghost_users: u64,
    pub expired_sessions: u64,
    pub expired_resets: u64,
    pub expired_webhooks: u64,
}

/// E-posta adresinin SHA-256 özetini üretir (imha kaydında açık e-posta tutulmaz)
pub fn hash_email(email: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(email.trim().to_lowercase().as_bytes());
    hex::encode(hasher.finalize())
}

/// Kullanıcıya ait tüm verileri atomik bir veritabanı işlemi (transaction) içinde
/// savunmacı (defense-in-depth) basamaklarla kalıcı olarak siler.
/// Hem SQLite yabancı anahtar (foreign key cascade) mekanizmasını hem de açık SQL silmelerini işletir.
pub async fn purge_user_records_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: &str,
) -> Result<(), AppError> {
    // 1. Kullanıcıya ait aktif/pasif tüm oturumları sil
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut **tx)
        .await?;

    // 2. Şifre sıfırlama taleplerini sil
    sqlx::query("DELETE FROM password_resets WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut **tx)
        .await?;

    // 3. Kullanıcının projelerine ait webhook olay kayıtlarını sil
    sqlx::query(
        "DELETE FROM webhook_events WHERE project_id IN (SELECT id FROM projects WHERE user_id = ?)",
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    // 4. Kullanıcının projelerine ait sürüm notlarını (entries) sil
    sqlx::query(
        "DELETE FROM entries WHERE project_id IN (SELECT id FROM projects WHERE user_id = ?)",
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    // 5. Kullanıcının projelerini sil
    sqlx::query("DELETE FROM projects WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut **tx)
        .await?;

    // 6. Kullanıcı kaydını sil
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(&mut **tx)
        .await?;

    Ok(())
}

/// Kullanıcının hesabını ve tüm ilişkili verilerini KVKK kapsamında kalıcı olarak siler
/// ve imha ledger'ına (hem SQLite hem data/erasure-ledger.jsonl) işler.
pub async fn purge_user(
    pool: &SqlitePool,
    user_id: &str,
    email: &str,
) -> Result<(), AppError> {
    let email_hash = hash_email(email);
    let now = chrono::Utc::now().to_rfc3339();
    let record_id = Uuid::new_v4().to_string();

    // 1. Veritabanında tüm ilişkili verileri atomik transaction ile kalıcı imha et
    let mut tx = pool.begin().await.map_err(AppError::Database)?;
    purge_user_records_tx(&mut tx, user_id).await?;

    // 2. İmha Ledger'ı veritabanı tablosuna kaydet
    sqlx::query(
        r#"
        INSERT INTO erasure_ledger (id, user_id, email_hash, purged_at, reason)
        VALUES (?, ?, ?, ?, 'kvkk_user_request')
        ON CONFLICT(email_hash) DO NOTHING
        "#,
    )
    .bind(&record_id)
    .bind(user_id)
    .bind(&email_hash)
    .bind(&now)
    .execute(&mut *tx)
    .await?;

    tx.commit().await.map_err(AppError::Database)?;

    // 3. Yerel data/erasure-ledger.jsonl dosyasına ekle
    let record = ErasureRecord {
        user_id: user_id.to_string(),
        email_hash: email_hash.clone(),
        purged_at: now,
        reason: "kvkk_user_request".to_string(),
    };

    if let Ok(line) = serde_json::to_string(&record) {
        if let Some(parent) = Path::new(LOCAL_LEDGER_PATH).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(LOCAL_LEDGER_PATH)
        {
            let _ = writeln!(file, "{}", line);
        }
    }

    tracing::info!("KVKK Hesap İmhası tamamlandı: user_id={}, email_hash={}", user_id, email_hash);
    Ok(())
}

/// Sunucu açılışında ve periyodik retention kontrollerinde imha ledger'ını okur;
/// eski bir sistem/veritabanı yedeğinden dönülmüş olabilecek "hayalet" (ghost)
/// kullanıcıları hem `user_id` hem `email_hash` üzerinden tespit ederek anında yeniden imha eder.
pub async fn apply_erasure_ledger(pool: &SqlitePool) -> u64 {
    if !Path::new(LOCAL_LEDGER_PATH).exists() {
        return 0;
    }

    let content = match std::fs::read_to_string(LOCAL_LEDGER_PATH) {
        Ok(c) => c,
        Err(_) => return 0,
    };

    let mut purged_count = 0;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Ok(record) = serde_json::from_str::<ErasureRecord>(trimmed) {
            // Hem user_id hem email_hash üzerinden tara (eski yedek restore edilmişse yakala)
            let matching_users: Vec<(String,)> = sqlx::query_as(
                "SELECT id FROM users WHERE id = ? OR email_hash = ?",
            )
            .bind(&record.user_id)
            .bind(&record.email_hash)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            for (uid,) in matching_users {
                tracing::warn!(
                    "⚠️ KVKK RETENTION KORUMASI: Eski yedekten dirilen kullanıcı (ID: {}) tespit edildi, anında imha ediliyor!",
                    uid
                );
                if let Ok(mut tx) = pool.begin().await {
                    if purge_user_records_tx(&mut tx, &uid).await.is_ok() {
                        let _ = tx.commit().await;
                        purged_count += 1;
                    }
                }
            }
        }
    }

    if purged_count > 0 {
        tracing::warn!("KVKK Retention Hook tamamlandı: {} adet hayalet kullanıcı imha edildi.", purged_count);
    } else {
        tracing::debug!("KVKK Retention kontrolü temiz: Eski yedekten dirilen kullanıcı yok.");
    }

    purged_count
}

/// Kapsamlı retention ve temizlik bakım işlemi:
/// 1. Hayalet kullanıcıları tespit edip temizler (Restore Retention Hook)
/// 2. Süresi dolmuş oturumları temizler (sessions tablosu)
/// 3. Süresi dolmuş veya kullanılmış şifre sıfırlama tokenlarını temizler
/// 4. 30 günden eski webhook etkinlik loglarını temizler
/// 5. SQLite sorgu planlayıcı ve boşluk optimizasyonunu tetikler (PRAGMA optimize)
pub async fn run_retention_cleanup(pool: &SqlitePool) -> Result<RetentionReport, AppError> {
    let ghost_purged = apply_erasure_ledger(pool).await;

    // Süresi dolmuş oturumları temizle
    let expired_sessions = sqlx::query("DELETE FROM sessions WHERE expires_at <= datetime('now')")
        .execute(pool)
        .await
        .map(|r| r.rows_affected())
        .unwrap_or(0);

    // Süresi dolmuş veya önceden kullanılmış şifre sıfırlama tokenlarını temizle
    let expired_resets = sqlx::query(
        "DELETE FROM password_resets WHERE expires_at <= datetime('now') OR used_at IS NOT NULL",
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0);

    // 30 günden eski webhook loglarını temizle (KVKK ve depolama hijyeni)
    let expired_webhooks = sqlx::query(
        "DELETE FROM webhook_events WHERE created_at < datetime('now', '-30 days')",
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0);

    // SQLite sorgu planlayıcısını optimize et
    let _ = sqlx::query("PRAGMA optimize").execute(pool).await;

    if ghost_purged > 0 || expired_sessions > 0 || expired_resets > 0 || expired_webhooks > 0 {
        tracing::info!(
            "Retention bakım raporu: {} hayalet kullanıcı, {} süresi dolmuş oturum, {} şifre sıfırlama kaydı, {} eski webhook kaydı temizlendi.",
            ghost_purged,
            expired_sessions,
            expired_resets,
            expired_webhooks
        );
    }

    Ok(RetentionReport {
        purged_ghost_users: ghost_purged,
        expired_sessions,
        expired_resets,
        expired_webhooks,
    })
}

/// Arka planda düzenli aralıklarla çalışan retention bakım döngüsü.
/// Sunucu açıkken eski bir SQLite dosyası yedekten dönülse dahi belirli aralıklarla
/// imha ledger'ını tarayıp hayalet kullanıcıları ve süresi dolmuş oturumları otomatik temizler.
pub fn spawn_retention_worker(pool: SqlitePool) {
    tokio::spawn(async move {
        // İlk kontrol: Sunucu açıldıktan 60 saniye sonra
        tokio::time::sleep(Duration::from_secs(60)).await;
        let _ = run_retention_cleanup(&pool).await;

        let mut interval = tokio::time::interval(Duration::from_secs(3600)); // Her saat başı
        loop {
            interval.tick().await;
            let _ = run_retention_cleanup(&pool).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{Entry, Project, User};
    use crate::db::{create_user, init_db, insert_entry, upsert_project};
    use chrono::Utc;

    #[tokio::test]
    async fn test_purge_user_cascade_cleans_everything() {
        let pool = init_db("sqlite::memory:", None).await.unwrap();
        let user_id = "user-kvkk-test";
        let email = "kvkk-purge@example.com";

        let user = User {
            id: user_id.to_string(),
            email: email.to_string(),
            email_hash: Some(hash_email(email)),
            password_hash: "hash".to_string(),
            name: Some("KVKK Test".to_string()),
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };
        create_user(&pool, &user, None).await.unwrap();

        // Oturum ekle
        sqlx::query("INSERT INTO sessions (id, user_id, expires_at) VALUES ('sess-1', ?, datetime('now', '+1 day'))")
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();

        // Proje ve Entry ekle
        let project = Project {
            id: "proj-kvkk".to_string(),
            user_id: Some(user_id.to_string()),
            github_repo_full_name: "test/kvkk".to_string(),
            name: "KVKK Project".to_string(),
            slug: "kvkk-project".to_string(),
            widget_key: "w_kvkk".to_string(),
            brand_name: None,
            brand_color: "#10b981".to_string(),
            brand_logo_url: None,
            webhook_secret: "sec".to_string(),
            parse_mode: "ai_editorial".to_string(),
            audience: "end_user".to_string(),
            template_style: "standard".to_string(),
            language: "auto".to_string(),
            is_private: 0,
            custom_github_token: None,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };
        upsert_project(&pool, &project).await.unwrap();

        let entry = Entry {
            id: "entry-kvkk".to_string(),
            project_id: "proj-kvkk".to_string(),
            category: "NEW".to_string(),
            title: "Test Entry".to_string(),
            body: "Body".to_string(),
            status: "PUBLISHED".to_string(),
            ai_generated: 1,
            source_commit_shas: "[]".to_string(),
            source_pr_number: None,
            author_username: None,
            published_at: None,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };
        insert_entry(&pool, &entry, None).await.unwrap();

        // Kullanıcıyı purge et
        purge_user(&pool, user_id, email).await.unwrap();

        // Doğrulamalar: Tüm tablolar sıfırlanmalı!
        let user_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(user_count.0, 0, "Kullanıcı silinmelidir");

        let sess_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(sess_count.0, 0, "Oturumlar silinmelidir");

        let proj_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM projects WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(proj_count.0, 0, "Projeler silinmelidir");

        let entry_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM entries WHERE project_id = 'proj-kvkk'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(entry_count.0, 0, "İlişkili sürüm notları silinmelidir");

        // erasure_ledger tablosunda imha kaydı bulunmalı
        let ledger_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM erasure_ledger WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(ledger_count.0, 1, "İmha ledger'ına kayıt düşmelidir");
    }

    #[tokio::test]
    async fn test_run_retention_cleanup_purges_expired_sessions_and_resets() {
        let pool = init_db("sqlite::memory:", None).await.unwrap();
        let user_id = "user-retention-test";

        let user = User {
            id: user_id.to_string(),
            email: "retention@example.com".to_string(),
            email_hash: Some(hash_email("retention@example.com")),
            password_hash: "hash".to_string(),
            name: Some("Retention Test".to_string()),
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };
        create_user(&pool, &user, None).await.unwrap();

        // 1 süresi geçmiş oturum, 1 aktif oturum ekle
        sqlx::query("INSERT INTO sessions (id, user_id, expires_at) VALUES ('sess-exp', ?, datetime('now', '-1 hour'))")
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sessions (id, user_id, expires_at) VALUES ('sess-act', ?, datetime('now', '+1 hour'))")
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();

        // 1 süresi geçmiş reset, 1 kullanılmış reset, 1 aktif reset ekle
        sqlx::query("INSERT INTO password_resets (id, user_id, token_hash, expires_at, used_at) VALUES ('rst-exp', ?, 'h1', datetime('now', '-1 hour'), NULL)")
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO password_resets (id, user_id, token_hash, expires_at, used_at) VALUES ('rst-used', ?, 'h2', datetime('now', '+1 hour'), datetime('now'))")
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO password_resets (id, user_id, token_hash, expires_at, used_at) VALUES ('rst-act', ?, 'h3', datetime('now', '+1 hour'), NULL)")
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();

        let report = run_retention_cleanup(&pool).await.unwrap();
        assert_eq!(report.expired_sessions, 1);
        assert_eq!(report.expired_resets, 2);

        // Kalan oturum sadece aktif olan olmalı
        let remaining_sessions: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(remaining_sessions.0, 1);

        // Kalan reset sadece aktif ve kullanılmamış olan olmalı
        let remaining_resets: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM password_resets")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(remaining_resets.0, 1);
    }
}
