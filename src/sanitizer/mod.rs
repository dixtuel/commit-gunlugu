use regex::Regex;
use std::sync::LazyLock;

static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+").unwrap()
});

static SIGNED_OFF_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(signed-off-by|co-authored-by):\s*([^<]+)<[^>]+>").unwrap()
});

/// Commit mesajları ve PR gövdelerindeki kişisel e-posta adreslerini,
/// imza kalıntılarını ve hassas kimlik verilerini temizler.
pub fn sanitize_text(input: &str) -> String {
    // 1. Signed-off-by / Co-authored-by içindeki e-postaları maskele
    let clean = SIGNED_OFF_REGEX.replace_all(input, "$1: $2");

    // 2. Kalan tüm e-posta adreslerini temizle
    EMAIL_REGEX.replace_all(&clean, "[gizlendi]").to_string()
}

/// Yazar adını güvenli hale getirir (e-posta içeriyorsa sadece kullanıcı adını bırakır)
pub fn sanitize_author(author: Option<&str>, username: Option<&str>) -> String {
    if let Some(user) = username {
        if !user.trim().is_empty() {
            return format!("@{}", user.trim().trim_start_matches('@'));
        }
    }

    if let Some(auth) = author {
        // "Name <email@domain.com>" formatındaki açılı parantezleri ve içini temizle
        let before_bracket = if let Some(idx) = auth.find('<') {
            &auth[..idx]
        } else {
            auth
        };

        let clean = sanitize_text(before_bracket);
        let trimmed = clean.trim();
        if !trimmed.is_empty() && trimmed != "[gizlendi]" {
            return trimmed.to_string();
        }
    }

    "Geliştirici".to_string()
}

/// KVKK ve GDPR uyumlu e-posta maskeleme.
/// Kişisel tanımlayıcı bilgileri (PII) gizler, yalnızca doğrulama için ilk ve son karakteri bırakır.
/// Örn: asrinklcc@sely.tr -> a***c@sely.tr
pub fn mask_email(email: &str) -> String {
    let trimmed = email.trim();
    let parts: Vec<&str> = trimmed.split('@').collect();
    if parts.len() != 2 {
        return "***".to_string();
    }
    let local = parts[0];
    let domain = parts[1];

    let chars: Vec<char> = local.chars().collect();
    let masked_local = match chars.len() {
        0 => "***".to_string(),
        1 => format!("{}***", chars[0]),
        2 => format!("{}***{}", chars[0], chars[1]),
        _ => format!("{}***{}", chars[0], chars[chars.len() - 1]),
    };

    format!("{}@{}", masked_local, domain)
}

/// KVKK ve GDPR uyumlu IP anonimleştirme.
/// İstemcinin tekil cihaz olarak profillenmesini önlemek için:
/// - IPv4 adreslerinin son oktetini sıfırlar (/24 subnet maskeleme, ör: 185.23.17.0/24)
/// - IPv6 adreslerinin son 80 bitini sıfırlar (/48 subnet maskeleme, ör: 2001:db8:85a3::/48)
pub fn anonymize_ip(ip_str: &str) -> String {
    use std::net::IpAddr;
    let trimmed = ip_str.trim();
    if let Ok(ip) = trimmed.parse::<IpAddr>() {
        match ip {
            IpAddr::V4(v4) => {
                let octets = v4.octets();
                format!("{}.{}.{}.0/24", octets[0], octets[1], octets[2])
            }
            IpAddr::V6(v6) => {
                let segments = v6.segments();
                format!("{:x}:{:x}:{:x}::/48", segments[0], segments[1], segments[2])
            }
        }
    } else {
        "anon".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_emails() {
        let msg = "fix: resolve login bug for user developer@example.com in auth module";
        let cleaned = sanitize_text(msg);
        assert_eq!(cleaned, "fix: resolve login bug for user [gizlendi] in auth module");
    }

    #[test]
    fn test_sanitize_signed_off() {
        let msg = "feat: add webhook validator\n\nSigned-off-by: Jane Doe <jane@example.com>\nCo-authored-by: Dev <dev@company.com>";
        let cleaned = sanitize_text(msg);
        assert!(!cleaned.contains("jane@example.com"));
        assert!(!cleaned.contains("dev@company.com"));
        assert!(cleaned.contains("Signed-off-by: Jane Doe"));
    }

    #[test]
    fn test_sanitize_author() {
        assert_eq!(sanitize_author(None, Some("octocat")), "@octocat");
        assert_eq!(sanitize_author(Some("Jane Doe <jane@example.com>"), None), "Jane Doe");
        assert_eq!(sanitize_author(None, None), "Geliştirici");
    }

    #[test]
    fn test_mask_email() {
        assert_eq!(mask_email("asrinklcc@sely.tr"), "a***c@sely.tr");
        assert_eq!(mask_email("user@example.com"), "u***r@example.com");
        assert_eq!(mask_email("a@example.com"), "a***@example.com");
        assert_eq!(mask_email("ab@example.com"), "a***b@example.com");
        assert_eq!(mask_email("invalid-email"), "***");
    }

    #[test]
    fn test_anonymize_ip() {
        assert_eq!(anonymize_ip("185.23.17.42"), "185.23.17.0/24");
        assert_eq!(anonymize_ip("127.0.0.1"), "127.0.0.0/24");
        assert_eq!(anonymize_ip("2001:0db8:85a3:0000:0000:8a2e:0370:7334"), "2001:db8:85a3::/48");
        assert_eq!(anonymize_ip("invalid-ip"), "anon");
    }
}
