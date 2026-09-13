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

/// E-posta adresinin SHA-256 özetini üretir (imha kaydında açık e-posta tutulmaz)
pub fn hash_email(email: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(email.trim().to_lowercase().as_bytes());
    hex::encode(hasher.finalize())
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

    // 1. Veritabanından kullanıcıyı ve ilişkili verileri sil (ON DELETE CASCADE)
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;

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
    .execute(pool)
    .await?;

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
/// kullanıcıları tespit ederek anında yeniden imha eder.
pub async fn apply_erasure_ledger(pool: &SqlitePool) {
    if !Path::new(LOCAL_LEDGER_PATH).exists() {
        return;
    }

    let content = match std::fs::read_to_string(LOCAL_LEDGER_PATH) {
        Ok(c) => c,
        Err(_) => return,
    };

    let mut purged_count = 0;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Ok(record) = serde_json::from_str::<ErasureRecord>(trimmed) {
            // Veritabanında bu user_id var mı kontrol et (eski backup restore edilmişse dirilmiş olabilir)
            let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM users WHERE id = ?")
                .bind(&record.user_id)
                .fetch_optional(pool)
                .await
                .unwrap_or(None);

            if let Some((uid,)) = exists {
                tracing::warn!(
                    "⚠️ KVKK RETENTION KORUMASI: Eski yedekten dirilen kullanıcı (ID: {}) tespit edildi, anında imha ediliyor!",
                    uid
                );
                let _ = sqlx::query("DELETE FROM users WHERE id = ?")
                    .bind(&uid)
                    .execute(pool)
                    .await;
                purged_count += 1;
            }
        }
    }

    if purged_count > 0 {
        tracing::warn!("KVKK Retention Hook tamamlandı: {} adet hayalet kullanıcı imha edildi.", purged_count);
    } else {
        tracing::debug!("KVKK Retention kontrolü temiz: Eski yedekten dirilen kullanıcı yok.");
    }
}

/// Arka planda düzenli aralıklarla çalışan retention bakım döngüsü.
/// Sunucu açıkken eski bir SQLite dosyası yedekten dönülse dahi belirli aralıklarla
/// imha ledger'ını tarayıp hayalet kullanıcıları otomatik olarak temizler.
pub fn spawn_retention_worker(pool: SqlitePool) {
    tokio::spawn(async move {
        // İlk kontrol: 5 dakika sonra
        tokio::time::sleep(Duration::from_secs(300)).await;
        let mut interval = tokio::time::interval(Duration::from_secs(3600)); // Her saat başı

        loop {
            interval.tick().await;
            apply_erasure_ledger(&pool).await;
        }
    });
}
