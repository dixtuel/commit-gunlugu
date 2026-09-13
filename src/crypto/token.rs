use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand::RngCore;

const ENC_PREFIX: &str = "enc:";

/// Kullanıcının kişisel GitHub PAT'ini AES-256-GCM ile şifreler.
/// `key_hex` 64 karakterlik (32 byte) hex-encoded bir anahtar olmalıdır
/// (`TOKEN_ENCRYPTION_KEY` ortam değişkeni). Anahtar tanımlı değilse
/// çağıran taraf şifrelemeyi atlayıp token'ı olduğu gibi saklamalıdır.
pub fn encrypt_token(plaintext: &str, key_hex: &str) -> Option<String> {
    let key_bytes = hex::decode(key_hex).ok()?;
    if key_bytes.len() != 32 {
        tracing::warn!("TOKEN_ENCRYPTION_KEY 32 byte (64 hex karakter) olmalı, şifreleme atlanıyor");
        return None;
    }
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);

    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes()).ok()?;
    Some(format!(
        "{}{}:{}",
        ENC_PREFIX,
        hex::encode(nonce_bytes),
        hex::encode(ciphertext)
    ))
}

/// Depolama öncesi kolaylık sarmalayıcısı: anahtar tanımlıysa şifreler,
/// tanımlı değilse veya şifreleme başarısız olursa düz metni olduğu gibi döndürür
/// (self-host / anahtarsız kurulumlarda özellik bozulmaz, sadece korumasız kalır).
pub fn encrypt_token_for_storage(plaintext: &str, key_hex: Option<&str>) -> String {
    match key_hex {
        Some(key) => encrypt_token(plaintext, key).unwrap_or_else(|| plaintext.to_string()),
        None => plaintext.to_string(),
    }
}

/// Şifrelenmiş bir token'ı çözer. `enc:` öneki yoksa (eski düz-metin kayıt
/// veya anahtar hiç tanımlanmamışsa) değeri olduğu gibi geri döndürür —
/// böylece mevcut kayıtlarla geriye dönük uyumluluk korunur.
pub fn decrypt_token(stored: &str, key_hex: Option<&str>) -> String {
    let Some(rest) = stored.strip_prefix(ENC_PREFIX) else {
        return stored.to_string();
    };

    let Some(key_hex) = key_hex else {
        tracing::warn!("Şifrelenmiş token bulundu ama TOKEN_ENCRYPTION_KEY tanımlı değil, çözülemedi");
        return stored.to_string();
    };

    (|| -> Option<String> {
        let (nonce_hex, ct_hex) = rest.split_once(':')?;
        let nonce_bytes = hex::decode(nonce_hex).ok()?;
        let ct = hex::decode(ct_hex).ok()?;
        let key_bytes = hex::decode(key_hex).ok()?;
        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let plaintext = cipher.decrypt(nonce, ct.as_ref()).ok()?;
        String::from_utf8(plaintext).ok()
    })()
    .unwrap_or_else(|| {
        tracing::error!("Token şifre çözme başarısız, anahtar değişmiş olabilir");
        String::new()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = hex::encode([7u8; 32]);
        let secret = "ghp_super_secret_token_123";
        let enc = encrypt_token(secret, &key).unwrap();
        assert!(enc.starts_with(ENC_PREFIX));
        assert_eq!(decrypt_token(&enc, Some(&key)), secret);
    }

    #[test]
    fn test_plaintext_passthrough_without_prefix() {
        let legacy = "ghp_old_plaintext_token";
        assert_eq!(decrypt_token(legacy, Some(&hex::encode([1u8; 32]))), legacy);
    }
}
