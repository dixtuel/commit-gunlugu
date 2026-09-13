use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub default_webhook_secret: String,
    pub nvidia_nim_api_key: Option<String>,
    pub nvidia_nim_models: Vec<String>,
    pub ai_api_base_url: Option<String>,
    pub ai_api_key: Option<String>,
    pub ai_model: Option<String>,
    pub app_url: String,
    pub github_token: Option<String>,
    pub webhook_rate_limit_per_minute: u64,
    pub api_rate_limit_per_minute: u64,
}

impl Config {
    pub fn from_env() -> Self {
        let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8095);

        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite://data/commit_gunlugu.db?mode=rwc".to_string());

        let default_webhook_secret = env::var("GITHUB_WEBHOOK_SECRET")
            .unwrap_or_else(|_| "commit_gunlugu_dev_secret_change_me_in_prod".to_string());

        let nvidia_nim_api_key = env::var("NVIDIA_NIM_API_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty());

        let nvidia_nim_models = env::var("NVIDIA_NIM_MODELS")
            .unwrap_or_else(|_| "deepseek-ai/deepseek-v4-flash-0731,nvidia/nemotron-3.5-lightning-30b-a3b".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let ai_api_base_url = env::var("AI_API_BASE_URL")
            .ok()
            .filter(|u| !u.trim().is_empty());
        let ai_api_key = env::var("AI_API_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty());
        let ai_model = env::var("AI_MODEL")
            .ok()
            .filter(|m| !m.trim().is_empty());

        let app_url = env::var("APP_BASE_URL")
            .unwrap_or_else(|_| "https://commit.dixtuel.tr".to_string());

        let github_token = env::var("GITHUB_TOKEN")
            .ok()
            .filter(|t| !t.trim().is_empty());

        let webhook_rate_limit_per_minute = env::var("WEBHOOK_RATE_LIMIT_PER_MINUTE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);

        let api_rate_limit_per_minute = env::var("API_RATE_LIMIT_PER_MINUTE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        Self {
            host,
            port,
            database_url,
            default_webhook_secret,
            nvidia_nim_api_key,
            nvidia_nim_models,
            ai_api_base_url,
            ai_api_key,
            ai_model,
            app_url,
            github_token,
            webhook_rate_limit_per_minute,
            api_rate_limit_per_minute,
        }
    }
}
