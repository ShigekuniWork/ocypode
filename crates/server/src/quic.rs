use std::{
    error::Error,
    net::SocketAddr,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use futures_util::SinkExt;
use s2n_quic::{Server, provider::endpoint_limits, stream::BidirectionalStream};
use tokio_stream::StreamExt;
use tokio_util::{
    codec::{FramedRead, FramedWrite},
    sync::CancellationToken,
};
use tracing::info;

use crate::{
    config::QuicConfig,
    error::{CodecError, ServerCodecError},
    parser::{Frame, ServerCodec, ServerOutbound, pb},
};

static CLIENT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Sends INFO to the client and awaits a CONNECT response within the given timeout.
/// INFO is also re-sent when server configuration is updated.
async fn perform_handshake<R, W>(
    framed_read: &mut FramedRead<R, ServerCodec>,
    framed_write: &mut FramedWrite<W, ServerCodec>,
    connect_timeout: Duration,
    client_id: u64,
) -> Result<pb::Connect, ServerCodecError>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let info =
        ServerOutbound::info(1, client_id, "ocypode-server".to_string(), "ocypode".to_string());
    framed_write.send(info).await?;

    tokio::time::timeout(connect_timeout, async {
        match framed_read.next().await {
            Some(Ok(Frame::Connect(msg))) => Ok(msg),
            Some(Err(e)) => Err(e),
            None => Err(ServerCodecError::Codec(CodecError::InvalidFormat(
                "connection closed before CONNECT".to_string(),
            ))),
        }
    })
    .await
    .map_err(|_| {
        ServerCodecError::Codec(CodecError::InvalidFormat("CONNECT message timeout".to_string()))
    })?
}

fn handle_frame(frame: Frame) -> Result<(), ServerCodecError> {
    match frame {
        Frame::Connect(_) => {
            info!("Received unexpected CONNECT after handshake");
        }
    }
    Ok(())
}

async fn handle_bidirectional_stream(
    stream: BidirectionalStream,
    connect_timeout: Duration,
    read_buffer_size: usize,
    write_buffer_size: usize,
) -> Result<(), ServerCodecError> {
    let (receive_stream, send_stream) = stream.split();
    let mut framed_read = FramedRead::with_capacity(receive_stream, ServerCodec, read_buffer_size);
    let mut framed_write = FramedWrite::with_capacity(send_stream, ServerCodec, write_buffer_size);

    let client_id = CLIENT_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    perform_handshake(&mut framed_read, &mut framed_write, connect_timeout, client_id).await?;
    info!("Accepted CONNECT from client_id={}", client_id);

    while let Some(frame) = framed_read.next().await {
        handle_frame(frame?)?;
    }

    Ok(())
}

pub async fn start(
    config: &QuicConfig,
    shutdown: CancellationToken,
) -> Result<SocketAddr, Box<dyn Error + Send + Sync>> {
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

    let local_addr = server.local_addr()?;
    info!("Ocypode server listening to {}", local_addr);

    let connect_timeout = Duration::from_millis(config.connect_timeout);
    let read_buffer_size = config.read_buffer_size;
    let write_buffer_size = config.write_buffer_size;
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("Ocypode server stopped gracefully");
                    break;
                }
                connection = server.accept() => {
                    if let Some(mut connection) = connection {
                        tokio::spawn(async move {
                            while let Ok(Some(stream)) = connection.accept_bidirectional_stream().await {
                                let timeout = connect_timeout;
                                tokio::spawn(async move {
                                    if let Err(error) = handle_bidirectional_stream(stream, timeout, read_buffer_size, write_buffer_size).await {
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures_util::SinkExt;
    use tokio_stream::StreamExt;
    use tokio_util::codec::{FramedRead, FramedWrite};

    use super::perform_handshake;
    use crate::{
        error::ServerCodecError,
        parser::{ClientCodec, ClientOutbound, ServerCodec},
    };

    #[tokio::test]
    async fn perform_handshake_sends_info_and_accepts_connect() {
        let (client_io, server_io) = tokio::io::duplex(4096);
        let (server_rx, server_tx) = tokio::io::split(server_io);
        let (client_rx, client_tx) = tokio::io::split(client_io);

        let server = async {
            let mut framed_read = FramedRead::with_capacity(server_rx, ServerCodec, 4096);
            let mut framed_write = FramedWrite::with_capacity(server_tx, ServerCodec, 4096);
            perform_handshake(&mut framed_read, &mut framed_write, Duration::from_secs(1), 1).await
        };

        let client = async {
            let mut framed_read = FramedRead::with_capacity(client_rx, ClientCodec, 4096);
            let info = framed_read.next().await.unwrap().unwrap();
            let crate::parser::ClientFrame::Info(info_msg) = info;
            assert_eq!(info_msg.client_id, 1);
            let mut framed_write = FramedWrite::with_capacity(client_tx, ClientCodec, 4096);
            framed_write.send(ClientOutbound::connect(1, false)).await.unwrap();
        };

        let (result, _): (Result<_, ServerCodecError>, _) = tokio::join!(server, client);
        result.unwrap();
    }
}
