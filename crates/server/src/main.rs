use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{
    config::{MetricLevel, ServerConfig},
    grpc::grpc_serve,
    logger::init_ocypode_logger,
    metrics::MetricsManager,
};

mod auth;
mod client;
mod config;
mod error;
mod grpc;
mod handshake;
mod logger;
mod metrics;
mod parser;
mod permission;
mod quic;
mod router;
mod topic;
mod transport;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Arc::new(ServerConfig::new());
    init_ocypode_logger(&config.logger);

    info!("Starting ocypode-server");

    let cancel_token = CancellationToken::new();

    // Setup gRPC server.
    grpc_serve(&config.grpc, cancel_token.clone()).await;

    // Setup metrics service.
    if config.metrics.metrics_level > MetricLevel::Disabled {
        MetricsManager::boot_metrics_service(
            config.metrics.listen_addr.clone(),
            cancel_token.clone(),
        );
    }

    // Start Ocypode Server
    let quic_addr = quic::start(Arc::clone(&config), cancel_token.clone()).await?;
    info!("QUIC server listening on {}", quic_addr);

    info!("Server is ready");

    tokio::signal::ctrl_c().await?;
    Ok(())
}
