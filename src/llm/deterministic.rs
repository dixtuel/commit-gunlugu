use serde::{Deserialize, Serialize};

use crate::sanitizer::sanitize_text;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryDraft {
    pub category: String, // NEW | FIX | IMPROVEMENT
    pub title: String,
    pub body: String,
}

/// Yapay zeka servislerine ulaşılamadığında veya API anahtarı girilmediğinde
/// Conventional Commits kurallarını analiz ederek sıfır-hata garantisiyle
/// tutarlı bir changelog taslağı üreten deterministik motor.
pub fn generate_deterministic_entry(
    pr_title: Option<&str>,
    pr_body: Option<&str>,
    commit_messages: &[String],
) -> EntryDraft {
    // PR başlığı varsa birincil kaynak odur; yoksa ilk anlamlı commit mesajı
    let primary_text = pr_title
        .or_else(|| commit_messages.first().map(|s| s.as_str()))
        .unwrap_or("Sistem ve kod tabanı güncellemeleri");

    let clean_primary = sanitize_text(primary_text);
    let lower = clean_primary.to_lowercase();

    // 1. Kategori tespiti
    let category = if lower.starts_with("feat") || lower.contains("add ") || lower.contains("yeni ") {
        "NEW".to_string()
    } else if lower.starts_with("fix") || lower.contains("bug") || lower.contains("düzelt") || lower.contains("hata") {
        "FIX".to_string()
    } else {
        "IMPROVEMENT".to_string()
    };

    // 2. Başlık temizliği (Conventional Commits prefixlerini kaldır)
    let raw_title = clean_primary
        .splitn(2, ':')
        .last()
        .unwrap_or(&clean_primary)
        .trim();

    let title = clean_title(raw_title);

    // 3. Gövde (body) oluşturma
    let body = if let Some(body_text) = pr_body {
        let clean_b = sanitize_text(body_text);
        let first_line = clean_b.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        if first_line.len() > 10 {
            first_line.chars().take(280).collect::<String>()
        } else {
            format!("{} kapsamında ilgili geliştirmeler tamamlandı ve yayına alındı.", title)
        }
    } else if commit_messages.len() > 1 {
        let count = commit_messages.len();
        format!(
            "Bu sürümde {} adet commit birleştirilerek kararlılık ve performans geliştirmeleri sağlandı.",
            count
        )
    } else {
        format!("{} ile ilgili kod değişiklikleri ve iyileştirmeler uygulandı.", title)
    };

    EntryDraft {
        category,
        title,
        body,
    }
}

fn clean_title(input: &str) -> String {
    let mut s = input.trim();

    // Sondaki PR referanslarını temizle: (#12)
    if let Some(idx) = s.rfind("(#") {
        if s.ends_with(')') {
            s = s[..idx].trim();
        }
    }

    if s.is_empty() {
        return "Sistem İyileştirmesi".to_string();
    }

    // İlk harfi büyüt
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feat_commit() {
        let commits = vec!["feat(auth): add google and github oauth login buttons (#42)".to_string()];
        let entry = generate_deterministic_entry(None, None, &commits);
        assert_eq!(entry.category, "NEW");
        assert_eq!(entry.title, "Add google and github oauth login buttons");
    }

    #[test]
    fn test_fix_commit() {
        let commits = vec!["fix: resolve database timeout issue during peak hours".to_string()];
        let entry = generate_deterministic_entry(None, None, &commits);
        assert_eq!(entry.category, "FIX");
        assert_eq!(entry.title, "Resolve database timeout issue during peak hours");
    }

    #[test]
    fn test_pr_priority() {
        let commits = vec!["chore: bump deps".to_string()];
        let pr_title = "feat: Yeni karanlık mod teması";
        let pr_body = "Kullanıcıların göz yorgunluğunu azaltan modern koyu tema eklendi.";
        let entry = generate_deterministic_entry(Some(pr_title), Some(pr_body), &commits);
        assert_eq!(entry.category, "NEW");
        assert_eq!(entry.title, "Yeni karanlık mod teması");
        assert!(entry.body.contains("Kullanıcıların"));
    }
}
