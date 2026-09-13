use std::sync::Arc;
use governor::middleware::NoOpMiddleware;
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor, GovernorLayer,
};

/// IP başına dakikada `per_minute` istekle sınırlı bir tower katmanı üretir.
/// Cloudflare arkasında (CF-Connecting-IP) ve doğrudan bağlantılarda
/// akıllı IP tespiti yaparak brute-force ve DDoS saldırılarını önler.
pub fn per_minute(per_minute: u64) -> GovernorLayer<SmartIpKeyExtractor, NoOpMiddleware> {
    let burst_size = per_minute.max(1) as u32;
    let period_ms = 60_000 / per_minute.max(1);

    let config = GovernorConfigBuilder::default()
        .key_extractor(SmartIpKeyExtractor)
        .per_millisecond(period_ms)
        .burst_size(burst_size)
        .finish()
        .expect("Governor rate-limit yapılandırması kurulamadı");

    GovernorLayer {
        config: Arc::new(config),
    }
}
