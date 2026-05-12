// This module *is* the centralized env reader, so the project-wide
// disallowed-methods rule against `std::env::var` doesn't apply here.
#![allow(clippy::disallowed_methods)]

use std::env;

use once_cell::sync::OnceCell;

#[derive(Debug, Clone)]
pub struct Config {
    pub dev_db: String,
    #[allow(dead_code)] // wired up by integration test harnesses, not main
    pub test_db: String,
    pub encryption_key: String,
    pub allowed_origins: Vec<String>,
    pub db_pool_size: u32,
    pub bind_address: String,
}

static CONFIG: OnceCell<Config> = OnceCell::new();

impl Config {
    /// Load configuration from environment (and `.env`, if present).
    /// Idempotent — safe to call from `main` and from tests.
    pub fn init() -> &'static Config {
        CONFIG.get_or_init(load)
    }

    /// Lazily initialize on first access. Use this from anywhere that needs
    /// config without forcing the caller to remember to call `init()` first
    /// (notably tests, which don't go through `main`).
    pub fn get() -> &'static Config {
        Self::init()
    }
}

fn load() -> Config {
    let _ = dotenvy::dotenv();

    Config {
        dev_db: env::var("DEV_DB").unwrap_or_default(),
        test_db: env::var("TEST_DB").unwrap_or_default(),
        encryption_key: require_var("ENCRYPTION_KEY"),
        allowed_origins: env::var("ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:8000".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        db_pool_size: env::var("DB_POOL_SIZE").ok().and_then(|s| s.parse().ok()).unwrap_or(16),
        bind_address: env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:3000".to_string()),
    }
}

fn require_var(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("environment variable `{name}` must be set"))
}
