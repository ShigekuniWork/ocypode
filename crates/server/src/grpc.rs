use std::net::SocketAddr;

use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::sync::CancellationToken;
use tonic_health::ServingStatus;
use tracing::{error, info};

use crate::config::GrpcConfig;

/// Bootstraps the Ocypode gRPC server.
///
/// Returns when the `shutdown` token is triggered.
pub async fn grpc_serve(config: &GrpcConfig, shutdown: CancellationToken) -> SocketAddr {
    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter.set_service_status("ocypode-service", ServingStatus::Serving).await;

    let listener = tokio::net::TcpListener::bind(config.socket_addr()).await.unwrap();
    let listen_addr = listener.local_addr().unwrap();

    let server = tonic::transport::Server::builder()
        .add_service(health_service)
        .serve_with_incoming(TcpListenerStream::new(listener));

    tokio::spawn(async move {
        tokio::select! {
            response = server => {
                if let Err(e) = response {
                    error!("Ocypose gRPC server error: {}", e)
                }
            }
            _ = shutdown.cancelled() => {
                info!("Ocypose gRPC server received shutdown signal");
            }
        }
    });

    info!("Ocypode gRPC server listening to {}", listen_addr);

    listen_addr
}
