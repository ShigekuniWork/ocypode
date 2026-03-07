use std::net::SocketAddr;

use bytes::BytesMut;
use s2n_quic::{Client, client::Connect};
use server::protocol::{ClientCodec, ClientInbound, ClientOutbound, ProtocolCodec};

/// Connect a QUIC client to the given server address and open a
/// bidirectional stream. Returns the stream ready for send/receive.
pub async fn connect_client(addr: SocketAddr) -> s2n_quic::stream::BidirectionalStream {
    let cert_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("certs/server.crt");

    let client = Client::builder()
        .with_tls(cert_path.as_path())
        .expect("failed to configure client TLS")
        .with_io("0.0.0.0:0")
        .expect("failed to bind client")
        .start()
        .expect("failed to start QUIC client");

    let connect = Connect::new(addr).with_server_name("localhost");
    let mut connection = client.connect(connect).await.expect("failed to connect");

    connection.open_bidirectional_stream().await.expect("failed to open stream")
}

/// Send a [`ClientOutbound`] message over the stream.
pub async fn send(stream: &mut s2n_quic::stream::BidirectionalStream, msg: &ClientOutbound) {
    let encoded = ClientCodec::encode(msg);
    stream.send(encoded.freeze()).await.expect("failed to send data");
}

/// Receive and decode one [`ClientInbound`] message from the stream.
/// Times out after 2 seconds to avoid hanging tests.
pub async fn recv(stream: &mut s2n_quic::stream::BidirectionalStream) -> ClientInbound {
    let mut buf = BytesMut::new();

    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            match ClientCodec::decode(&mut buf) {
                Ok(msg) => return msg,
                Err(server::protocol::DecodeError::Incomplete) => {}
                Err(e) => panic!("decode error: {e:?}"),
            }

            match stream.receive().await {
                Ok(Some(data)) => buf.extend_from_slice(&data),
                Ok(None) => panic!("stream closed before a full message was received"),
                Err(e) => panic!("receive error: {e}"),
            }
        }
    })
    .await
    .expect("timed out waiting for server response")
}
