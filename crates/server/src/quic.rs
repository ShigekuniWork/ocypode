use std::{error::Error, net::SocketAddr, time::Duration};

use bytes::BytesMut;
use s2n_quic::{
    Server,
    provider::endpoint_limits,
    stream::{BidirectionalStream, ReceiveStream, SendStream},
};
use tokio::io::AsyncWriteExt;
use tokio_stream::StreamExt;
use tokio_util::{
    codec::{Encoder, FramedRead},
    sync::CancellationToken,
};
use tracing::info;

use crate::{
    config::QuicConfig,
    error::{CodecError, ServerCodecError},
    parser::{Frame, ServerCodec, ServerOutbound, pb},
};

/// Sends INFO to the client and awaits a CONNECT response within the given timeout.
/// INFO is also re-sent when server configuration is updated.
async fn perform_handshake(
    framed_read: &mut FramedRead<ReceiveStream, ServerCodec>,
    send_stream: &mut SendStream,
    connect_timeout: Duration,
) -> Result<pb::Connect, ServerCodecError> {
    let mut output_buffer = BytesMut::new();
    let mut codec = ServerCodec;
    codec.encode(ServerOutbound::default_info(), &mut output_buffer)?;
    send_stream.write_all(&output_buffer).await?;

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
        Frame::Connect(msg) => {
            info!("Received unexpected CONNECT from client_id={}", msg.client_id);
        }
    }
    Ok(())
}

async fn handle_bidirectional_stream(
    stream: BidirectionalStream,
    connect_timeout: Duration,
    read_buffer_size: usize,
) -> Result<(), ServerCodecError> {
    let (receive_stream, mut send_stream) = stream.split();
    let mut framed_read = FramedRead::with_capacity(receive_stream, ServerCodec, read_buffer_size);

    let connect_msg =
        perform_handshake(&mut framed_read, &mut send_stream, connect_timeout).await?;
    info!("Accepted CONNECT from client_id={}", connect_msg.client_id);

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
                                    if let Err(error) = handle_bidirectional_stream(stream, timeout, read_buffer_size).await {
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
