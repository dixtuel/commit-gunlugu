use minijinja::Environment;
use std::sync::Arc;

use crate::config::Config;
use crate::db::DbPool;
use crate::llm::client::LlmFallbackEngine;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub llm: LlmFallbackEngine,
    pub config: Config,
    pub jinja: Arc<Environment<'static>>,
}

impl AppState {
    pub fn new(
        db: DbPool,
        llm: LlmFallbackEngine,
        config: Config,
        jinja: Environment<'static>,
    ) -> Self {
        Self {
            db,
            llm,
            config,
            jinja: Arc::new(jinja),
        }
    }
}
