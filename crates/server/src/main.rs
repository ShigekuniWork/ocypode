use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{
    config::{MetricLevel, ServerConfig},
    grpc::grpc_serve,
    logger::init_ocypode_logger,
    metrics::MetricsManager,
};

mod config;
mod grpc;
mod logger;
mod metrics;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ServerConfig::new();
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

    info!("Server is ready");

    tokio::signal::ctrl_c().await?;
    Ok(())
}
