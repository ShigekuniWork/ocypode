use std::{net::UdpSocket, path::Path, sync::Arc, time::Duration};

use bytes::BytesMut;
use s2n_quic::{Client, client::Connect};
use server::{
    config::QuicConfig,
    error::ClientCodecError,
    parser::{ClientCodec, ClientFrame, ClientOutbound, CommandCodec, ServerOutbound, pb},
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio_util::{
    codec::{Decoder, Encoder},
    sync::CancellationToken,
};

type TestError = Box<dyn std::error::Error + Send + Sync>;

#[allow(dead_code)]
fn reserve_udp_port() -> std::io::Result<u16> {
    let socket = UdpSocket::bind("127.0.0.1:0")?;
    socket.local_addr().map(|addr| addr.port())
}

fn default_info_message() -> pb::Info {
    ServerOutbound::default_info()
}

fn sample_connect_message() -> pb::Connect {
    ClientOutbound::connect(1, true)
}

async fn read_next_client_frame<ReceiveStream>(
    receive_stream: &mut ReceiveStream,
    incoming_bytes: &mut BytesMut,
) -> Result<Option<ClientFrame>, ClientCodecError>
where
    ReceiveStream: AsyncRead + Unpin,
{
    let mut client_codec = ClientCodec;
    loop {
        if let Some(frame) = client_codec.decode(incoming_bytes)? {
            return Ok(Some(frame));
        }

        let bytes_read = receive_stream.read_buf(incoming_bytes).await?;
        if bytes_read == 0 {
            return Ok(None);
        }
    }
}

async fn write_client_frame<SendStream, Message>(
    send_stream: &mut SendStream,
    message: Message,
) -> Result<(), ClientCodecError>
where
    SendStream: AsyncWrite + Unpin,
    Message: CommandCodec,
{
    let mut client_codec = ClientCodec;
    let mut output_buffer = BytesMut::new();
    client_codec.encode(message, &mut output_buffer)?;
    send_stream.write_all(&output_buffer).await?;
    Ok(())
}

async fn setup_server_and_client(
    connect_timeout: u64,
) -> Result<(Arc<QuicConfig>, CancellationToken, s2n_quic::Client, std::net::SocketAddr), TestError>
{
    let mut quic_config = QuicConfig {
        enable_gso: false,
        enable_gro: false,
        listen_addr: "127.0.0.1:0".to_string(),
        connect_timeout,
        ..Default::default()
    };
    quic_config.tls.cert_file_path = "../certs/server.crt".to_string();
    quic_config.tls.key_file_path = "../certs/key.pem".to_string();

    let cancellation_token = CancellationToken::new();
    let server_config = Arc::new(quic_config);
    let server_shutdown = cancellation_token.clone();

    let server_address = server::quic::start(&server_config, server_shutdown).await?;

    let client = Client::builder()
        .with_tls(Path::new("../certs/server.crt"))?
        .with_io("0.0.0.0:0")?
        .start()?;

    Ok((server_config, cancellation_token, client, server_address))
}

#[tokio::test]
async fn info_then_connect_over_quic() -> Result<(), TestError> {
    let (_server_config, cancellation_token, client, server_address) =
        setup_server_and_client(5).await?;

    let connect = Connect::new(server_address).with_server_name("localhost");
    let mut connection = client.connect(connect).await?;
    connection.keep_alive(true)?;

    let stream = connection.open_bidirectional_stream().await?;
    let (mut receive_stream, mut send_stream) = stream.split();

    // Receive INFO from server using ClientCodec (from parser.rs)
    let mut incoming_bytes = BytesMut::new();
    let info_message =
        match read_next_client_frame(&mut receive_stream, &mut incoming_bytes).await? {
            Some(ClientFrame::Info(message)) => message,
            None => {
                return Err(Box::from(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "missing INFO",
                )));
            }
        };

    // Verify INFO message content
    let expected_info = default_info_message();
    assert_eq!(info_message.version, expected_info.version);
    assert_eq!(info_message.auth_type, expected_info.auth_type);
    assert_eq!(info_message.server_id, expected_info.server_id);
    assert_eq!(info_message.server_name, expected_info.server_name);
    assert_eq!(info_message.max_payload, expected_info.max_payload);
    assert!(info_message.client_id > 0, "server must assign a non-zero client_id");

    // Send CONNECT to server using ClientCodec (from parser.rs)
    let connect_message = sample_connect_message();
    write_client_frame(&mut send_stream, connect_message.clone()).await?;

    // Verify connection is established by sending another CONNECT
    tokio::time::sleep(Duration::from_millis(100)).await;
    write_client_frame(&mut send_stream, connect_message).await?;

    send_stream.close().await?;

    cancellation_token.cancel();

    Ok(())
}

#[tokio::test]
async fn connect_timeout_when_no_connect_message() -> Result<(), TestError> {
    let (_server_config, cancellation_token, client, server_address) =
        setup_server_and_client(1).await?;

    let connect = Connect::new(server_address).with_server_name("localhost");
    let mut connection = client.connect(connect).await?;
    connection.keep_alive(true)?;

    let stream = connection.open_bidirectional_stream().await?;
    let (mut receive_stream, mut _send_stream) = stream.split();

    // Receive INFO from server
    let mut incoming_bytes = BytesMut::new();
    let _info_message =
        match read_next_client_frame(&mut receive_stream, &mut incoming_bytes).await? {
            Some(ClientFrame::Info(message)) => message,
            None => {
                return Err(Box::from(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "missing INFO",
                )));
            }
        };

    // Don't send CONNECT message - just wait for the stream to be closed due to timeout
    tokio::time::sleep(Duration::from_secs(2)).await;

    // The server should have closed the connection due to timeout
    let bytes_read = receive_stream.read_buf(&mut incoming_bytes).await?;
    assert_eq!(bytes_read, 0, "Expected stream to be closed by server due to timeout");

    cancellation_token.cancel();

    Ok(())
}
