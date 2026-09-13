use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use uuid::Uuid;

use crate::error::AppError;

const LOCAL_LEDGER_PATH: &str = "data/erasure-ledger.jsonl";
const R2_REMOTE_DEST: &str = "r2-mikoshi-crypt:latest/commit-gunlugu-erasure-ledger.jsonl";

#[derive(serde::Serialize, serde::Deserialize, Debug)]
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

/// Kullanıcının hesabını ve tüm ilişkili verilerini KVKK kapsamında kalıcı olarak siler,
/// imha ledger'ına işler ve R2 nesne deposuna bağımsız olarak senkronize eder.
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

    // 4. Asenkron olarak R2'ye kopyala (Arka plan task'ı, kullanıcıyı bekletmez)
    tokio::spawn(async move {
        sync_ledger_to_r2().await;
    });

    tracing::info!("KVKK Hesap İmhası tamamlandı: user_id={}, email_hash={}", user_id, email_hash);
    Ok(())
}

/// R2 nesne deposuna imha ledger'ını senkronize eder
async fn sync_ledger_to_r2() {
    if !Path::new(LOCAL_LEDGER_PATH).exists() {
        return;
    }

    let status = tokio::task::spawn_blocking(|| {
        std::process::Command::new("rclone")
            .args(["copyto", LOCAL_LEDGER_PATH, R2_REMOTE_DEST])
            .status()
    })
    .await;

    match status {
        Ok(Ok(s)) if s.success() => {
            tracing::info!("İmha ledger'ı başarıyla R2'ye senkronize edildi ({})", R2_REMOTE_DEST);
        }
        Ok(Ok(s)) => {
            tracing::warn!("R2 imha ledger senkronizasyonu hata verdi (çıkış kodu: {:?})", s.code());
        }
        Ok(Err(e)) => {
            tracing::debug!("rclone çalıştırılamadı (yerel ledger güncel): {}", e);
        }
        Err(e) => {
            tracing::debug!("spawn_blocking hatası: {}", e);
        }
    }
}

/// Sunucu açılışında R2'den en güncel imha ledger'ını indirir ve
/// eski bir yedekten dönülmüş olabilecek "hayalet" (ghost) kullanıcıları tekrar imha eder.
pub async fn apply_erasure_ledger_on_startup(pool: &SqlitePool) {
    // 1. R2'den en güncel ledger'ı çekmeyi dene
    let download = tokio::task::spawn_blocking(|| {
        std::process::Command::new("rclone")
            .args(["copyto", R2_REMOTE_DEST, LOCAL_LEDGER_PATH])
            .status()
    })
    .await;

    if let Ok(Ok(s)) = download {
        if s.success() {
            tracing::info!("R2'den en güncel imha ledger'ı başarıyla indirildi.");
        }
    }

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
                    "⚠️ KVKK KURTARMA KORUMASI: Eski yedekten dirilen kullanıcı (ID: {}) tespit edildi, anında imha ediliyor!",
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
        tracing::warn!("KVKK Restore Hook tamamlandı: {} adet hayalet kullanıcı imha edildi.", purged_count);
    } else {
        tracing::info!("KVKK Restore Hook kontrolü temiz: Eski yedekten dirilen kullanıcı yok.");
    }
}
