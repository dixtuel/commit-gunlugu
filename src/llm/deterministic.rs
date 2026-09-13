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

/// Git-cliff ve Conventional Commits standardına uygun katı ayrıştırıcı
pub fn generate_conventional_entry(
    commit_messages: &[String],
) -> EntryDraft {
    let raw = commit_messages.first().map(|s| s.as_str()).unwrap_or("chore: kod tabanı güncellemesi");
    let clean = sanitize_text(raw);

    // Conventional commits regex mantığı: type(scope)!: description
    let (category, formatted_title) = if let Some(colon_idx) = clean.find(':') {
        let prefix = &clean[..colon_idx].trim();
        let desc = clean[colon_idx + 1..].trim();

        let cat = if prefix.starts_with("feat") {
            "NEW"
        } else if prefix.starts_with("fix") {
            "FIX"
        } else {
            "IMPROVEMENT"
        };

        // Scope ayıkla: feat(api)! -> [Api]
        let title = if let (Some(start), Some(end)) = (prefix.find('('), prefix.find(')')) {
            if end > start + 1 {
                let scope = &prefix[start + 1..end];
                format!("({}) {}", clean_title(scope), clean_title(desc))
            } else {
                clean_title(desc)
            }
        } else {
            clean_title(desc)
        };

        (cat.to_string(), title)
    } else {
        ("IMPROVEMENT".to_string(), clean_title(&clean))
    };

    let body = if commit_messages.len() > 1 {
        format!("Bu sürümde {} adet ilgili commit birleştirildi.", commit_messages.len())
    } else {
        format!("{} geliştirmesi tamamlandı.", formatted_title)
    };

    EntryDraft {
        category,
        title: formatted_title,
        body,
    }
}

/// Release-drafter benzeri PR etiketleri ve başlığı odaklı ayrıştırıcı
pub fn generate_pr_centric_entry(
    pr_title: &str,
    pr_body: Option<&str>,
    labels: &[String],
) -> Option<EntryDraft> {
    // skip-changelog etiketi varsa atla
    if labels.iter().any(|l| l.to_lowercase() == "skip-changelog" || l.to_lowercase() == "ignore") {
        return None;
    }

    let category = if labels.iter().any(|l| l.to_lowercase().contains("feature") || l.to_lowercase().contains("enhancement")) {
        "NEW".to_string()
    } else if labels.iter().any(|l| l.to_lowercase().contains("bug") || l.to_lowercase().contains("fix")) {
        "FIX".to_string()
    } else {
        "IMPROVEMENT".to_string()
    };

    let title = clean_title(pr_title);
    let body = pr_body
        .map(|b| sanitize_text(b).lines().take(3).collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| format!("{} PR'ı başarıyla ana dala birleştirildi.", title));

    Some(EntryDraft {
        category,
        title,
        body,
    })
}

/// Auto-changelog benzeri doğrudan ham commit formatlayıcı
pub fn generate_raw_git_entry(
    commit_messages: &[String],
    commit_shas: &[String],
) -> EntryDraft {
    let first_msg = commit_messages.first().cloned().unwrap_or_else(|| "Güncelleme".to_string());
    let title = clean_title(&first_msg);

    let sha_summary = if !commit_shas.is_empty() {
        let short_shas: Vec<String> = commit_shas
            .iter()
            .map(|s| if s.len() >= 7 { s[..7].to_string() } else { s.clone() })
            .collect();
        format!("Commitler: {}", short_shas.join(", "))
    } else {
        "Kaynak commit detayı bulunmuyor.".to_string()
    };

    EntryDraft {
        category: "IMPROVEMENT".to_string(),
        title,
        body: sha_summary,
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

    #[test]
    fn test_conventional_mode() {
        let commits = vec!["feat(billing): add stripe and shopier webhook support".to_string()];
        let entry = generate_conventional_entry(&commits);
        assert_eq!(entry.category, "NEW");
        assert!(entry.title.contains("(Billing)"));
        assert!(entry.title.contains("Add stripe and shopier"));
    }

    #[test]
    fn test_pr_centric_mode() {
        let labels = vec!["enhancement".to_string(), "ui".to_string()];
        let entry = generate_pr_centric_entry("Yeni filtreleme paneli", Some("Açıklama"), &labels);
        assert!(entry.is_some());
        let e = entry.unwrap();
        assert_eq!(e.category, "NEW");

        // skip-changelog testi
        let skip_labels = vec!["skip-changelog".to_string()];
        let skipped = generate_pr_centric_entry("Dahili refactor", None, &skip_labels);
        assert!(skipped.is_none());
    }

    #[test]
    fn test_raw_git_mode() {
        let commits = vec!["quick bugfix in payment gateway".to_string()];
        let shas = vec!["9e78222faeac31d0e7102".to_string()];
        let entry = generate_raw_git_entry(&commits, &shas);
        assert_eq!(entry.category, "IMPROVEMENT");
        assert!(entry.body.contains("9e78222"));
    }
}
