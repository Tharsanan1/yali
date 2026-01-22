//! AI-Native Gateway (Yali) - Binary entry point.
//!
//! This is the main entry point for the gateway binary.

use pingora_core::prelude::*;
use pingora_proxy::http_proxy_service;
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use yali_gateway::config::GatewayConfig;
use yali_gateway::proxy::AIGatewayProxy;
use yali_gateway::state::GatewayState;

fn main() {
    // Initialize logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive("yali_gateway=info".parse().unwrap()))
        .init();

    info!("Starting AI-Native Gateway (Yali)");

    // Load configuration
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.json".to_string());

    let config = match GatewayConfig::from_file(&config_path) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to load configuration from '{}': {}", config_path, e);
            std::process::exit(1);
        }
    };

    // Validate configuration
    if let Err(e) = config.validate() {
        error!("Configuration validation failed: {}", e);
        std::process::exit(1);
    }

    info!(
        providers = config.providers.len(),
        backends = config.backends.len(),
        routes = config.routes.len(),
        "Configuration loaded"
    );

    // Create gateway state
    let state = Arc::new(GatewayState::new());
    if let Err(e) = state.load_config(&config) {
        error!("Failed to load configuration into state: {}", e);
        std::process::exit(1);
    }

    // Create Pingora server
    let mut server = Server::new(None).unwrap();
    server.bootstrap();

    // Create the proxy service
    let proxy = AIGatewayProxy::new(state);
    let mut proxy_service = http_proxy_service(&server.configuration, proxy);

    // Get listen address from environment or use default
    let listen_addr =
        std::env::var("GATEWAY_LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    info!(addr = %listen_addr, "Starting HTTP proxy service");

    proxy_service.add_tcp(&listen_addr);

    server.add_service(proxy_service);

    info!("Gateway is running on {}", listen_addr);
    server.run_forever();
}
