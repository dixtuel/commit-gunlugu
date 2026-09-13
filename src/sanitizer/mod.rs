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
}
