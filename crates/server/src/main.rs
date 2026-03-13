use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{config::ServerConfig, grpc::grpc_serve, logger::init_ocypode_logger};

mod config;
mod grpc;
mod logger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ServerConfig::new();
    init_ocypode_logger(&config.logger);

    info!("Starting ocypode-server");

    let cancel_token = CancellationToken::new();

    // Setup gRPC server.
    grpc_serve(&config.grpc, cancel_token).await;

    info!("Server is ready");

    tokio::signal::ctrl_c().await?;
    Ok(())
}
