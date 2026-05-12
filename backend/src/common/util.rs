// Test-only helper kept for backwards-compatibility with the existing test
// suite. The application code goes through `crate::common::config::Config`.
#![allow(clippy::disallowed_methods)]

use std::env;
use std::sync::Once;

static DOTENV_INIT: Once = Once::new();

/// Read an environment variable, loading `.env` on first call.
/// Panics if the variable is not set — intended for tests where missing env
/// means the harness is broken.
#[allow(dead_code)]
pub fn load_environment_variable(variable_name: &str) -> String {
    DOTENV_INIT.call_once(|| {
        let _ = dotenvy::dotenv();
    });
    env::var(variable_name).unwrap_or_else(|_| panic!("{variable_name} must be set"))
}
