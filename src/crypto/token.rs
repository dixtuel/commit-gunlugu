use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand::RngCore;

const ENC_PREFIX: &str = "enc:";
const ENC_ZSTD_PREFIX: &str = "enc:zstd:";
const COMPRESSION_THRESHOLD: usize = 512;

fn encrypt_bytes_with_prefix(prefix: &str, data: &[u8], key_hex: &str) -> Option<String> {
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

    let ciphertext = cipher.encrypt(nonce, data).ok()?;
    Some(format!(
        "{}{}:{}",
        prefix,
        hex::encode(nonce_bytes),
        hex::encode(ciphertext)
    ))
}

fn decrypt_bytes(rest: &str, key_hex: &str) -> Option<Vec<u8>> {
    let (nonce_hex, ct_hex) = rest.split_once(':')?;
    let nonce_bytes = hex::decode(nonce_hex).ok()?;
    let ct = hex::decode(ct_hex).ok()?;
    let key_bytes = hex::decode(key_hex).ok()?;
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&nonce_bytes);
    cipher.decrypt(nonce, ct.as_ref()).ok()
}

/// Kullanıcının kişisel GitHub PAT'ini AES-256-GCM ile şifreler.
/// `key_hex` 64 karakterlik (32 byte) hex-encoded bir anahtar olmalıdır
/// (`TOKEN_ENCRYPTION_KEY` ortam değişkeni). Anahtar tanımlı değilse
/// çağıran taraf şifrelemeyi atlayıp token'ı olduğu gibi saklamalıdır.
pub fn encrypt_token(plaintext: &str, key_hex: &str) -> Option<String> {
    encrypt_bytes_with_prefix(ENC_PREFIX, plaintext.as_bytes(), key_hex)
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

    decrypt_bytes(rest, key_hex)
        .and_then(|b| String::from_utf8(b).ok())
        .unwrap_or_else(|| {
            tracing::error!("Token şifre çözme başarısız, anahtar değişmiş olabilir");
            String::new()
        })
}

/// Uzun metinleri (changelog gövdesi, açıklamalar vb.) önce Zstandard (seviye 3) ile sıkıştırır
/// (eğer >= 512 byte ise ve boyut kazancı sağlıyorsa), ardından AES-256-GCM ile şifreler.
/// Sıkıştırma CPU ve RAM'i minimum derecede kullanır (~40 mikro-saniye).
pub fn pack_text_for_storage(plaintext: &str, key_hex: Option<&str>) -> String {
    let plain_bytes = plaintext.as_bytes();

    let Some(key_hex) = key_hex else {
        return plaintext.to_string();
    };

    // Eşik kontrolü: 512 byte veya daha büyük metinlerde Zstandard sıkıştırma dene
    if plain_bytes.len() >= COMPRESSION_THRESHOLD {
        // Seviye 3: CPU/RAM dostu ultra hızlı mod
        if let Ok(compressed) = zstd::encode_all(plain_bytes, 3) {
            // Defansif kontrol: Sıkıştırma en az 16 byte net kazanç sağladıysa enc:zstd: ile sakla
            if compressed.len() + 16 < plain_bytes.len() {
                if let Some(enc) = encrypt_bytes_with_prefix(ENC_ZSTD_PREFIX, &compressed, key_hex) {
                    return enc;
                }
            }
        }
    }

    // Eşiğin altındaysa veya sıkıştırma kazanç sağlamadıysa düz şifrele
    encrypt_bytes_with_prefix(ENC_PREFIX, plain_bytes, key_hex).unwrap_or_else(|| plaintext.to_string())
}

/// Veritabanından okunan metni şeffaf şekilde çözer:
/// 1. `enc:zstd:` öneki varsa: AES-256-GCM ile şifresini çözer, ardından Zstd ile decompress eder.
/// 2. `enc:` öneki varsa: Sadece AES-256-GCM ile şifresini çözer (sıkıştırılmamış kayıtlar).
/// 3. Önek yoksa: Düz metin olarak aynen geri döner (şifresiz eski kayıtlar).
pub fn unpack_text_from_storage(stored: &str, key_hex: Option<&str>) -> String {
    if let Some(rest) = stored.strip_prefix(ENC_ZSTD_PREFIX) {
        let Some(key_hex) = key_hex else {
            tracing::warn!("enc:zstd: formatında veri bulundu ancak TOKEN_ENCRYPTION_KEY tanımlı değil");
            return stored.to_string();
        };

        match decrypt_bytes(rest, key_hex) {
            Some(decrypted_bytes) => {
                match zstd::decode_all(decrypted_bytes.as_slice()) {
                    Ok(decompressed_bytes) => {
                        String::from_utf8(decompressed_bytes).unwrap_or_else(|_| stored.to_string())
                    }
                    Err(e) => {
                        tracing::error!("Zstandard decompress hatası: {}", e);
                        stored.to_string()
                    }
                }
            }
            None => {
                tracing::error!("enc:zstd şifre çözme hatası");
                stored.to_string()
            }
        }
    } else if let Some(rest) = stored.strip_prefix(ENC_PREFIX) {
        let Some(key_hex) = key_hex else {
            tracing::warn!("enc: formatında veri bulundu ancak TOKEN_ENCRYPTION_KEY tanımlı değil");
            return stored.to_string();
        };

        match decrypt_bytes(rest, key_hex) {
            Some(decrypted_bytes) => {
                String::from_utf8(decrypted_bytes).unwrap_or_else(|_| stored.to_string())
            }
            None => {
                tracing::error!("enc: şifre çözme hatası");
                stored.to_string()
            }
        }
    } else {
        stored.to_string()
    }
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

    #[test]
    fn test_pack_unpack_short_text_no_zstd() {
        let key = hex::encode([5u8; 32]);
        let short = "Kısa başlık veya not (512 byte'tan az)";
        let packed = pack_text_for_storage(short, Some(&key));
        assert!(packed.starts_with(ENC_PREFIX));
        assert!(!packed.starts_with(ENC_ZSTD_PREFIX));
        let unpacked = unpack_text_from_storage(&packed, Some(&key));
        assert_eq!(unpacked, short);
    }

    #[test]
    fn test_pack_unpack_long_text_with_zstd() {
        let key = hex::encode([9u8; 32]);
        let long = "Bu uzun bir sürüm notudur. İçerisinde birçok detaylı açıklama, güvenlik iyileştirmeleri ve hata düzeltmeleri yer almaktadır. ".repeat(15);
        assert!(long.len() > 1000);

        let packed = pack_text_for_storage(&long, Some(&key));
        assert!(packed.starts_with(ENC_ZSTD_PREFIX), "1 KB'den büyük metin Zstandard ile sıkıştırılmalıdır");

        let unpacked = unpack_text_from_storage(&packed, Some(&key));
        assert_eq!(unpacked, long, "Sıkıştırılmış ve şifrelenmiş metin hatasız geri açılmalıdır");
    }

    #[test]
    fn test_unpack_legacy_passthrough() {
        let key = hex::encode([3u8; 32]);
        let legacy_plain = "Veritabanındaki eski açık metin sürüm notu";
        assert_eq!(unpack_text_from_storage(legacy_plain, Some(&key)), legacy_plain);
    }
}
