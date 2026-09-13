use axum::{
    middleware as axum_mw,
    routing::{get, post},
    Router,
};
use minijinja::path_loader;
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod auth;
mod config;
mod crypto;
mod db;
mod email;
mod error;
mod llm;
mod middleware;
mod routes;
mod sanitizer;
mod state;

use config::Config;
use db::init_db;
use llm::client::LlmFallbackEngine;
use state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "commit_gunlugu=debug,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    tracing::info!("Yapılandırma yüklendi, port: {}", config.port);

    // 1. Veritabanı, migrasyon ve otomatik şifreleme başlatıcı
    let db_pool = init_db(&config.database_url, config.token_encryption_key.as_deref()).await?;

    // 2. KVKK İmha Ledger'ı & Restore Retention Hook'u
    // (Eski bir sistem yedeğinden dönülmüşse, silinen kullanıcıları otomatik tekrar imha eder)
    auth::erasure::apply_erasure_ledger(&db_pool).await;
    auth::erasure::spawn_retention_worker(db_pool.clone());

    // 3. Minijinja şablon motoru
    let mut jinja_env = minijinja::Environment::new();
    jinja_env.set_loader(path_loader("templates"));
    jinja_env.add_global("smtp_enabled", config.is_smtp_configured());

    // 4. AI Fallback motoru (NVIDIA NIM -> Gateway -> Deterministik)
    let llm_engine = LlmFallbackEngine::new(config.clone());

    let app_state = AppState::new(db_pool, llm_engine, config.clone(), jinja_env);

    // Rate Limiting katmanları (Leaky-Bucket)
    let webhook_limiter = middleware::rate_limit::per_minute(config.webhook_rate_limit_per_minute);
    let api_limiter = middleware::rate_limit::per_minute(config.api_rate_limit_per_minute);

    // CORS yapılandırması
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Webhook rotaları
    let webhook_routes = Router::new()
        .route("/api/v1/webhook", post(routes::webhook::handle_github_webhook))
        .layer(webhook_limiter);

    // Gömülebilir Widget rotaları
    let widget_routes = Router::new()
        .route("/api/v1/widget/:widget_key", get(routes::api::get_widget_data))
        .layer(api_limiter);

    // API rotaları
    let api_routes = Router::new()
        .route("/health", get(routes::api::health_check))
        .route("/api/v1/entries/:id/publish", post(routes::api::publish_entry_handler))
        .route("/api/v1/entries/:id/dismiss", post(routes::api::dismiss_entry_handler))
        .route("/api/v1/entries/:id/delete", post(routes::api::delete_entry_handler))
        .route("/api/v1/projects", get(routes::api::list_projects_handler).post(routes::api::create_project_handler))
        .route("/api/v1/projects/:id/settings", post(routes::api::update_project_settings_handler))
        .route("/api/v1/projects/:id/delete", post(routes::api::delete_project_handler))
        .route("/api/v1/projects/:id/entries", post(routes::api::create_manual_entry_handler))
        .route("/api/v1/projects/:id/sync-github", post(routes::api::sync_github_commits_handler))
        .route("/api/user/profile", post(routes::auth::update_profile_handler))
        .route("/api/user/change-password", post(routes::auth::change_password_handler))
        .route("/api/user/delete-account", post(routes::auth::delete_account_handler));

    // Kimlik Doğrulama (Auth) rotaları
    let auth_routes = Router::new()
        .route("/login", get(routes::auth::login_page).post(routes::auth::login_submit))
        .route("/register", get(routes::auth::register_page).post(routes::auth::register_submit))
        .route("/logout", get(routes::auth::logout_handler).post(routes::auth::logout_handler))
        .route("/forgot-password", get(routes::auth::forgot_password_page).post(routes::auth::forgot_password_submit))
        .route("/reset-password", get(routes::auth::reset_password_page).post(routes::auth::reset_password_submit));

    // Yasal ve SEO rotaları
    let legal_and_seo_routes = Router::new()
        .route("/terms", get(routes::legal::terms_page))
        .route("/privacy", get(routes::legal::privacy_page))
        .route("/robots.txt", get(routes::seo::robots_txt))
        .route("/sitemap.xml", get(routes::seo::sitemap_xml))
        .route("/google:token.html", get(routes::seo::google_verification));

    // Herkese Açık ve Dışa Aktarma rotaları
    let public_routes = Router::new()
        .route("/", get(routes::dashboard::landing_page))
        .route("/dashboard", get(routes::dashboard::dashboard_page))
        .route("/c/:slug", get(routes::public::public_changelog_page))
        .route("/c/:slug/export.md", get(routes::export::export_markdown_handler))
        .route("/c/:slug/feed.xml", get(routes::export::export_rss_handler))
        .route("/c/:slug/feed.json", get(routes::export::export_json_handler));

    let app = Router::new()
        .merge(public_routes)
        .merge(auth_routes)
        .merge(legal_and_seo_routes)
        .merge(webhook_routes)
        .merge(widget_routes)
        .merge(api_routes)
        .nest_service("/static", ServeDir::new("static"))
        .layer(cors)
        .layer(axum_mw::from_fn(middleware::security::security_headers))
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let addr: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .expect("Geçersiz HOST veya PORT adresi");

    tracing::info!("Commit Günlüğü başlatıldı: http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Ctrl+C dinleyici kurulamadı");
    tracing::info!("Kapatma sinyali alındı, sunucu sonlandırılıyor...");
}
