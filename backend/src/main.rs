use std::net::SocketAddr;

use axum::http::{HeaderValue, Method};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

mod common;
mod empires;
mod locations;
mod schema;
mod users;

use crate::common::{config::Config, db::create_shared_connection_pool};
use crate::empires::router::empires_route;
use crate::locations::router::locations_route;
use crate::users::router::users_route;

// Re-exports kept for the test modules that reference `crate::*`.
#[cfg(test)]
pub use crate::common::db::create_shared_connection_pool as _re_create_pool;
#[cfg(test)]
pub use crate::common::util::load_environment_variable as _re_load_env;

#[tokio::main]
async fn main() {
    fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let cfg = Config::init();
    info!(pool_size = cfg.db_pool_size, "loaded configuration");

    let shared_connection_pool = create_shared_connection_pool(cfg.dev_db.clone(), cfg.db_pool_size);

    let cors = build_cors(&cfg.allowed_origins);

    let app = users_route(shared_connection_pool.clone())
        .nest("/", locations_route(shared_connection_pool.clone()))
        .nest("/", empires_route(shared_connection_pool))
        .layer(cors);

    let addr: SocketAddr = cfg
        .bind_address
        .parse()
        .unwrap_or_else(|_| panic!("Invalid BIND_ADDRESS: {}", cfg.bind_address));
    info!(%addr, "listening");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("server crashed");
}

fn build_cors(allowed: &[String]) -> CorsLayer {
    let methods = [Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS];
    let headers = [
        http::header::AUTHORIZATION,
        http::header::CONTENT_TYPE,
        http::header::ACCEPT,
    ];

    let layer = CorsLayer::new().allow_methods(methods).allow_headers(headers);

    if allowed.iter().any(|o| o == "*") {
        // Explicit opt-in to wide-open CORS for local development only.
        layer.allow_origin(AllowOrigin::any())
    } else {
        let origins: Vec<HeaderValue> = allowed.iter().filter_map(|o| HeaderValue::from_str(o).ok()).collect();
        layer.allow_origin(AllowOrigin::list(origins))
    }
}
