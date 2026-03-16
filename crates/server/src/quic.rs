use std::{error::Error, net::SocketAddr, sync::Arc};

use s2n_quic::{Server, provider::endpoint_limits, stream::BidirectionalStream};
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{
    auth::{Authenticator, NoAuthAuthenticator},
    client::{Client, ClientError},
    config::ServerConfig,
    transport::Transport,
};

impl Transport for BidirectionalStream {
    type Reader = s2n_quic::stream::ReceiveStream;
    type Writer = s2n_quic::stream::SendStream;

    fn into_split(self) -> (Self::Reader, Self::Writer) {
        self.split()
    }
}

async fn handle_bidirectional_stream(
    stream: BidirectionalStream,
    config: Arc<ServerConfig>,
    authenticator: Arc<dyn Authenticator>,
) -> Result<(), ClientError> {
    let client = Client::new(stream, authenticator, config);
    client.run().await
}

pub async fn start(
    config: Arc<ServerConfig>,
    shutdown: CancellationToken,
) -> Result<SocketAddr, Box<dyn Error + Send + Sync>> {
    let addr: SocketAddr = config.quic.socket_addr();

    let io = s2n_quic::provider::io::Default::builder()
        .with_receive_address(addr)?
        .with_gso(config.quic.enable_gso)?
        .with_gro(config.quic.enable_gro)?
        .build()?;

    let endpoint_limits_config = if let Some(limit) = config.quic.endpoint_limits {
        endpoint_limits::Default::builder().with_inflight_handshake_limit(limit)?.build()?
    } else {
        endpoint_limits::Default::default()
    };

    let tls = {
        let tls_builder = s2n_quic::provider::tls::default::Server::builder().with_certificate(
            config.quic.tls.cert_file_path()?,
            config.quic.tls.key_file_path()?,
        )?;
        if config.tls_verify {
            tls_builder.with_client_authentication()?.build()?
        } else {
            tls_builder.build()?
        }
    };
    let mut server = Server::builder()
        .with_tls(tls)?
        .with_io(io)?
        .with_endpoint_limits(endpoint_limits_config)?
        .start()?;

    let local_addr = server.local_addr()?;
    info!("Ocypode server listening to {}", local_addr);

    let authenticator: Arc<dyn Authenticator> = Arc::new(NoAuthAuthenticator);

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("Ocypode server stopped gracefully");
                    break;
                }
                connection = server.accept() => {
                    if let Some(mut connection) = connection {
                        let config = Arc::clone(&config);
                        let authenticator = Arc::clone(&authenticator);
                        tokio::spawn(async move {
                            while let Ok(Some(stream)) = connection.accept_bidirectional_stream().await {
                                let config = Arc::clone(&config);
                                let auth = Arc::clone(&authenticator);
                                tokio::spawn(async move {
                                    if let Err(error) = handle_bidirectional_stream(stream, config, auth).await {
                                        info!("QUIC stream error: {}", error);
                                    }
                                });
                            }
                        });
                    } else {
                        break;
                    }
                }
            }
        }
    });

    Ok(local_addr)
}
