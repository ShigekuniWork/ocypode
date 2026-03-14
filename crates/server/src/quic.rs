use std::{error::Error, net::SocketAddr};

use s2n_quic::{Server, provider::endpoint_limits};
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::config::QuicConfig;

pub async fn start(config: &QuicConfig, shutdown: CancellationToken) -> Result<(), Box<dyn Error>> {
    let addr: SocketAddr = config.socket_addr();

    let io = s2n_quic::provider::io::Default::builder()
        .with_receive_address(addr)?
        .with_gso(config.enable_gso)?
        .with_gro(config.enable_gro)?
        .build()?;

    let endpoint_limits_config = if let Some(limit) = config.endpoint_limits {
        endpoint_limits::Default::builder().with_inflight_handshake_limit(limit)?.build()?
    } else {
        endpoint_limits::Default::default()
    };

    let mut server = Server::builder()
        .with_tls((config.tls.cert_file_path()?, config.tls.key_file_path()?))?
        .with_io(io)?
        .with_endpoint_limits(endpoint_limits_config)?
        .start()?;

    info!("Ocypode server listening to {}", addr);

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                info!("Ocypode server stopped gracefully");
                break;
            }
            connection = server.accept() => {
                if let Some(mut connection) = connection {
                    tokio::spawn(async move {
                        while let Ok(Some(mut _stream)) = connection.accept_bidirectional_stream().await {
                            // TODO: implement quic task
                            todo!()
                        }
                    });
                } else {
                    break;
                }
            }
        }
    }

    Ok(())
}
