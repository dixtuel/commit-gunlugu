use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: String,
    pub email: String,
    pub email_hash: Option<String>,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub name: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub expires_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PasswordReset {
    pub id: String,
    pub user_id: String,
    pub token_hash: String,
    pub expires_at: String,
    pub used_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Project {
    pub id: String,
    pub user_id: Option<String>,
    pub github_repo_full_name: String,
    pub name: String,
    pub slug: String,
    pub widget_key: String,
    pub brand_name: Option<String>,
    pub brand_color: String,
    pub brand_logo_url: Option<String>,
    pub webhook_secret: String,
    pub parse_mode: String,       // ai_editorial | conventional | pr_centric | raw_git
    pub audience: String,         // end_user | developer
    pub template_style: String,   // standard | grouped | compact
    pub language: String,         // auto | tr | en
    pub tracked_branch: String,   // newline-separated selected branches; empty = all branches
    pub is_private: i64,          // 0 = public, 1 = private
    #[serde(skip_serializing)]
    pub custom_github_token: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Project {
    pub fn tracked_branches(&self) -> Vec<String> {
        self.tracked_branch
            .split(['\n', '\r', ','])
            .map(str::trim)
            .filter(|branch| !branch.is_empty())
            .map(str::to_string)
            .collect()
    }

    pub fn tracks_branch(&self, branch: &str) -> bool {
        let clean = branch.strip_prefix("refs/heads/").unwrap_or(branch).trim();
        let selected = self.tracked_branches();
        selected.is_empty() || selected.iter().any(|item| item == clean)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Entry {
    pub id: String,
    pub project_id: String,
    pub category: String, // NEW | FIX | IMPROVEMENT
    pub title: String,
    pub body: String,
    pub status: String, // DRAFT | PUBLISHED | DISMISSED
    pub ai_generated: i64,
    pub source_commit_shas: String,
    pub source_pr_number: Option<i64>,
    pub author_username: Option<String>,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WebhookEvent {
    pub id: String,
    pub project_id: Option<String>,
    pub github_delivery_id: String,
    pub event_type: String,
    pub payload_summary: String,
    pub processed_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetPayload {
    pub brand: WidgetBrand,
    pub changelog_url: String,
    pub entries: Vec<WidgetEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetBrand {
    pub name: String,
    pub color: String,
    pub logo_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetEntry {
    pub id: String,
    pub category: String,
    pub title: String,
    pub body: String,
    pub author: Option<String>,
    pub published_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_tracked_branches_parsing() {
        let mut proj = Project {
            id: "1".into(),
            user_id: None,
            github_repo_full_name: "test/repo".into(),
            name: "test".into(),
            slug: "test".into(),
            widget_key: "k".into(),
            brand_name: None,
            brand_color: "#000".into(),
            brand_logo_url: None,
            webhook_secret: "s".into(),
            parse_mode: "ai_editorial".into(),
            audience: "end_user".into(),
            template_style: "standard".into(),
            language: "auto".into(),
            tracked_branch: "".into(),
            is_private: 0,
            custom_github_token: None,
            created_at: "".into(),
            updated_at: "".into(),
        };

        // Boş branch: her şeyi izler
        assert!(proj.tracked_branches().is_empty());
        assert!(proj.tracks_branch("main"));
        assert!(proj.tracks_branch("refs/heads/feature-1"));

        // Tek branch
        proj.tracked_branch = "main".into();
        assert_eq!(proj.tracked_branches(), vec!["main"]);
        assert!(proj.tracks_branch("main"));
        assert!(proj.tracks_branch("refs/heads/main"));
        assert!(!proj.tracks_branch("dev"));

        // Çoklu branch (alt alta / satır satır)
        proj.tracked_branch = "main\nbeta\nv1.x".into();
        assert_eq!(proj.tracked_branches(), vec!["main", "beta", "v1.x"]);
        assert!(proj.tracks_branch("main"));
        assert!(proj.tracks_branch("beta"));
        assert!(proj.tracks_branch("refs/heads/v1.x"));
        assert!(!proj.tracks_branch("release"));

        // Çoklu branch (virgülle ayrılmış)
        proj.tracked_branch = "main, develop, staging".into();
        assert_eq!(proj.tracked_branches(), vec!["main", "develop", "staging"]);
        assert!(proj.tracks_branch("main"));
        assert!(proj.tracks_branch("develop"));
        assert!(proj.tracks_branch("staging"));
        assert!(!proj.tracks_branch("prod"));
    }
}

