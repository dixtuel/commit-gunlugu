use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: String,
    pub email: String,
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
    pub created_at: String,
    pub updated_at: String,
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
