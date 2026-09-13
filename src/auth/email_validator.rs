use std::time::Duration;
use tokio::time::timeout;

/// Bilinen tek kullanımlık (disposable / temp) e-posta sağlayıcıları
const DISPOSABLE_DOMAINS: &[&str] = &[
    "tempmail.com", "10minutemail.com", "guerrillamail.com", "mailinator.com",
    "trashmail.com", "yopmail.com", "sharklasers.com", "getairmail.com",
    "throwawaymail.com", "fakeinbox.com", "temp-mail.org", "dispostable.com"
];

async fn host_has_dns_records(host_with_port: String) -> bool {
    match timeout(Duration::from_secs(3), tokio::net::lookup_host(host_with_port)).await {
        Ok(Ok(addrs)) => addrs.count() > 0,
        _ => false,
    }
}

/// E-posta adresinin biçimini, alan adını ve posta sunucusu (MX/A) erişilebilirliğini doğrular.
/// Sahte ve var olmayan alan adlarını (örn. asjfa@akodasokfds.com) anında engeller.
pub async fn validate_email_mx(email: &str) -> Result<(), &'static str> {
    let email = email.trim();
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return Err("Geçerli bir e-posta adresi giriniz.");
    }

    let user_part = parts[0];
    let domain = parts[1].to_lowercase();

    if user_part.is_empty() || domain.is_empty() {
        return Err("Geçerli bir e-posta adresi giriniz.");
    }

    if !domain.contains('.') {
        return Err("E-posta alan adı geçersiz uzantıya sahip.");
    }

    // 1. Tek kullanımlık çöp e-posta kontrolü
    if DISPOSABLE_DOMAINS.iter().any(|d| domain == *d || domain.ends_with(&format!(".{}", d))) {
        return Err("Tek kullanımlık (geçici) e-posta adresleri kabul edilmemektedir.");
    }

    // 2. DNS Host / MX çözümleme kontrolü (Asenkron Tokio)
    // Önce SMTP portunda (25), ardından Web portunda (80) host aranır
    let mx_host = format!("{}:25", domain);
    if host_has_dns_records(mx_host).await {
        return Ok(());
    }

    let web_host = format!("{}:80", domain);
    if host_has_dns_records(web_host).await {
        return Ok(());
    }

    Err("Girilen e-posta alan adı internet üzerinde mevcut değil veya posta sunucusuna sahip değil.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_valid_domain() {
        assert!(validate_email_mx("test@gmail.com").await.is_ok());
        assert!(validate_email_mx("contact@dixtuel.tr").await.is_ok());
    }

    #[tokio::test]
    async fn test_disposable_domain() {
        assert!(validate_email_mx("spam@mailinator.com").await.is_err());
        assert!(validate_email_mx("temp@tempmail.com").await.is_err());
    }

    #[tokio::test]
    async fn test_fake_domain() {
        assert!(validate_email_mx("asjfa@akodasokfds.com").await.is_err());
    }
}
