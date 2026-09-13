use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use password_hash::phc::PasswordHash;
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::error::AppError;

/// Şifreyi OWASP standartlarında Argon2id ile hashler.
pub fn hash_password(password: &str) -> Result<String, AppError> {
    let argon2 = Argon2::default();

    argon2
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("Şifre hashleme hatası: {}", e)))
}

/// Girilen şifreyi kayıtlı Argon2id hash ile doğrular. Sabit zamanlı karşılaştırır.
pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

/// Şifre sıfırlama için 32-byte kriptografik rastgele ham token ve veritabanında
/// saklanacak SHA-256 özetini üretir. (Veritabanı sızsa dahi token kullanılamaz).
pub fn generate_reset_token() -> (String, String) {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let raw_token = hex::encode(bytes);

    let mut hasher = Sha256::new();
    hasher.update(raw_token.as_bytes());
    let token_hash = hex::encode(hasher.finalize());

    (raw_token, token_hash)
}

/// Verilen ham token'ın SHA-256 hash'ini hesaplar.
pub fn hash_token(raw_token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw_token.as_bytes());
    hex::encode(hasher.finalize())
}

/// Kriptografik güvenli oturum anahtarı üretir.
pub fn generate_session_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// E-posta adresini arama yapılabilir kör indeks (searchable blind index) için
/// normalize edip SHA-256 ile özetler. Veritabanında açık e-posta saklanmaz.
pub fn hash_email(email: &str) -> String {
    let normalized = email.trim().to_lowercase();
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_password_hash_and_verify() {
        let pass = "SuperSecret123!";
        let hash = hash_password(pass).unwrap();
        assert!(verify_password(pass, &hash));
        assert!(!verify_password("WrongPassword", &hash));
    }

    #[test]
    fn test_reset_token() {
        let (raw, hash) = generate_reset_token();
        assert_eq!(raw.len(), 64);
        assert_eq!(hash.len(), 64);
        assert_eq!(hash_token(&raw), hash);
    }
}
