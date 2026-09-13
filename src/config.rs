use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub default_webhook_secret: String,
    pub nvidia_nim_api_key: Option<String>,
    pub nvidia_nim_models: Vec<String>,
    pub app_url: String,
    pub github_token: Option<String>,
    pub webhook_rate_limit_per_minute: u64,
    pub api_rate_limit_per_minute: u64,
    /// Kullanıcıların kişisel GitHub PAT'lerini DB'de şifrelemek için kullanılan
    /// 64 hex karakterlik (32 byte) AES-256-GCM anahtarı. Tanımlı değilse tokenlar
    /// düz metin olarak saklanır (geliştirme/self-host varsayılanı).
    pub token_encryption_key: Option<String>,
    /// Şifre sıfırlama e-postası göndermek için isteğe bağlı SMTP ayarları.
    /// R2_ERASURE_REMOTE ile aynı ilke: bu açık kaynak proje hiçbir altyapıya
    /// (ör. belirli bir SMTP sunucusu/domain) varsayılan olarak bağımlı DEĞİLDİR.
    /// `SMTP_HOST` tanımlı değilse e-posta gönderimi tamamen atlanır ve
    /// sıfırlama bağlantısı yalnızca sunucu logunda görünür — kendi SMTP
    /// bilgilerinizi (kendi mail sunucunuz, SendGrid, Postmark vb.) girerseniz
    /// gerçek e-posta gönderimi devreye girer.
    pub smtp_host: Option<String>,
    pub smtp_port: u16,
    pub smtp_from: String,
    /// KVKK/Gizlilik Politikası sayfasında "veri sorumlusu" olarak gösterilecek
    /// isim. Bu proje açık kaynak olduğu için gerçek bir kimlik GÖMÜLMEZ —
    /// tanımlı değilse jenerik bir placeholder gösterilir; kendi dağıtımınızda
    /// `.env`'e gerçek ad/unvanınızı yazmanız gerekir.
    pub legal_entity_name: String,
    /// (İsteğe bağlı) Veri hakları talepleri için gösterilecek iletişim e-postası.
    /// Tanımlı değilse yalnızca GitHub Issues bağlantısı gösterilir, gerçek bir
    /// e-posta adresi public sayfalarda/arama motorlarında ifşa edilmez.
    pub privacy_contact_email: Option<String>,
    /// Demo ve önizleme stüdyosunda gösterilecek varsayılan widget anahtarı.
    pub demo_widget_key: Option<String>,
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
            .unwrap_or_else(|_| "deepseek-ai/deepseek-v4-flash-0731,nvidia/nemotron-3.5-lightning-30b-a3b,google/gemma-4-31b-it".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

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

        let token_encryption_key = env::var("TOKEN_ENCRYPTION_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty());

        let smtp_host = env::var("SMTP_HOST")
            .ok()
            .filter(|h| !h.trim().is_empty());
        let smtp_port = env::var("SMTP_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(25);
        let smtp_from = env::var("SMTP_FROM").unwrap_or_else(|_| "no-reply@localhost".to_string());

        let legal_entity_name = env::var("LEGAL_ENTITY_NAME")
            .ok()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| "Proje Sahibi (kendi bilgilerinizi LEGAL_ENTITY_NAME ile tanımlayın)".to_string());

        let privacy_contact_email = env::var("PRIVACY_CONTACT_EMAIL")
            .ok()
            .filter(|e| !e.trim().is_empty());

        let demo_widget_key = env::var("DEMO_WIDGET_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty());

        Self {
            host,
            port,
            database_url,
            default_webhook_secret,
            nvidia_nim_api_key,
            nvidia_nim_models,
            app_url,
            github_token,
            webhook_rate_limit_per_minute,
            api_rate_limit_per_minute,
            token_encryption_key,
            smtp_host,
            smtp_port,
            smtp_from,
            legal_entity_name,
            privacy_contact_email,
            demo_widget_key,
        }
    }

    pub fn is_smtp_configured(&self) -> bool {
        self.smtp_host
            .as_deref()
            .map(|h| !h.trim().is_empty())
            .unwrap_or(false)
    }
}
