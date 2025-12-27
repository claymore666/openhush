//! REST API server for OpenHush daemon.
//!
//! Provides HTTP endpoints for remote control and integration with external tools
//! like Home Assistant, Node-RED, or custom web dashboards.
//!
//! # Security
//!
//! - Disabled by default
//! - Localhost only by default
//! - API key authentication required for all endpoints except health check
//! - CORS restricted by default
//!
//! # Usage
//!
//! Enable in config:
//! ```toml
//! [api]
//! enabled = true
//! bind = "127.0.0.1:8080"
//! swagger_ui = true
//! ```
//!
//! Generate an API key:
//! ```bash
//! openhush api-key generate
//! ```

mod auth;
mod handlers;
mod routes;
pub mod state;

pub use auth::{generate_api_key, hash_api_key};
pub use routes::create_router;
pub use state::{ApiCommand, ApiState, DaemonStatus};

use crate::config::ApiConfig;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::{error, info};

/// Start the API server.
pub async fn serve(state: ApiState, config: &ApiConfig) -> anyhow::Result<()> {
    let addr: SocketAddr = config
        .bind
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid API bind address '{}': {}", config.bind, e))?;

    let router = create_router(state, config);

    info!("Starting REST API server on {}", addr);
    if config.swagger_ui {
        info!("Swagger UI available at http://{}/swagger-ui/", addr);
    }

    let listener = TcpListener::bind(addr).await?;

    axum::serve(listener, router).await.map_err(|e| {
        error!("API server error: {}", e);
        anyhow::anyhow!("API server error: {}", e)
    })
}
