use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::error::AppError;

type HmacSha256 = Hmac<Sha256>;

/// GitHub Webhook X-Hub-Signature-256 doğrulamasını sabit zamanlı (constant-time)
/// karşılaştırma ile gerçekleştirir. Zamanlama saldırılarını (timing attack) önler.
pub fn verify_github_signature(
    secret: &str,
    signature_header: Option<&str>,
    body: &[u8],
) -> Result<(), AppError> {
    let signature_header = signature_header
        .ok_or_else(|| AppError::Unauthorized("X-Hub-Signature-256 başlığı eksik".to_string()))?;

    let hex_sig = signature_header
        .strip_prefix("sha256=")
        .ok_or_else(|| AppError::Unauthorized("Geçersiz imza formatı (sha256= bekleniyor)".to_string()))?;

    let expected_bytes = hex::decode(hex_sig)
        .map_err(|_| AppError::Unauthorized("İmza hex çözülemedi".to_string()))?;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| AppError::Internal(format!("HMAC başlatılamadı: {}", e)))?;

    mac.update(body);
    let computed_bytes = mac.finalize().into_bytes();

    if computed_bytes.as_slice().ct_eq(&expected_bytes).into() {
        Ok(())
    } else {
        Err(AppError::Unauthorized("Geçersiz webhook imzası".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_signature() {
        let secret = "test_secret_123";
        let body = b"{\"action\":\"push\",\"ref\":\"refs/heads/main\"}";

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let sig_hex = hex::encode(mac.finalize().into_bytes());
        let header = format!("sha256={}", sig_hex);

        let result = verify_github_signature(secret, Some(&header), body);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_signature() {
        let secret = "test_secret_123";
        let body = b"{\"action\":\"push\"}";
        let bad_header = "sha256=0000000000000000000000000000000000000000000000000000000000000000";

        let result = verify_github_signature(secret, Some(bad_header), body);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_header() {
        let secret = "test_secret_123";
        let body = b"test";

        let result = verify_github_signature(secret, None, body);
        assert!(result.is_err());
    }
}
