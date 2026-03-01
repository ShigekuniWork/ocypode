//! Minimal QUIC server using compio (placeholder implementation).

use anyhow::Result;
use bytes::{Bytes, BytesMut};
use common::wire::WireEncode;
use compio::{
    net::ToSocketAddrsAsync,
    quic::{Endpoint, ServerBuilder},
};
use protocol::{
    Message, ServerCodec,
    error::EncodeError,
    message::{Info, Msg},
};
use rustls_pki_types::PrivateKeyDer;
use topic::Topic;
use tracing::info;

const ALPN: &[u8] = b"ocypode";

// -----------------------------------------------------------------------------
// Server responses (fakes)
// -----------------------------------------------------------------------------
// TODO: Replace with proper implementation: load server identity, max_payload,
// and capabilities from configuration; generate server_id from instance/cluster
// identity.
/// Returns a fake INFO message to send to the client right after QUIC connect.
#[allow(dead_code)]
pub fn fake_info() -> Info {
    Info {
        version: 1,
        max_payload: 1_048_576,
        server_id: Bytes::from_static(b"ocypode-fake"),
        server_name: Bytes::from_static(b"ocypode"),
        auth_required: false,
        headers: true,
    }
}

/// Encodes the fake INFO message into a wire frame ready to send.
#[allow(dead_code)]
pub fn encoded_fake_info() -> Result<Bytes, EncodeError> {
    ServerCodec.encode(&Message::Info(fake_info()))
}

// TODO: Replace with proper implementation: PONG codec is not yet in the
// protocol crate. Once added, respond to client PING with a PONG frame.
// pub fn encoded_fake_pong() -> Result<Bytes, EncodeError> { ... }

// TODO: Replace with proper implementation: OK codec is not yet in the
// protocol crate. Once added, send OK for each received message when the
// client connected with verbose=true.
// pub fn encoded_ok() -> Result<Bytes, EncodeError> { ... }

// TODO: Replace with proper implementation: ERR codec is not yet in the
// protocol crate. Once added, send ERR on protocol or auth errors.
// pub fn encoded_err(...) -> Result<Bytes, EncodeError> { ... }

/// Builds a Topic from raw bytes (wire format: length-prefixed u16).
#[allow(dead_code)]
fn topic_from_bytes(raw: &[u8]) -> Topic {
    let mut buf = BytesMut::new();
    buf.put_length_prefixed_u16(raw);
    Topic::decode(&mut buf.freeze()).expect("valid topic")
}

// TODO: Replace with proper implementation: build Msg from real subscription
// routing (match Pub to Sub by topic, resolve subscription_id from session state).
/// Returns a fake MSG for delivery to a subscriber (e.g. for tests or stub flow).
#[allow(dead_code)]
pub fn fake_msg(topic_bytes: &[u8], subscription_id: Bytes, payload: Bytes) -> Msg {
    Msg {
        topic: topic_from_bytes(topic_bytes),
        subscription_id,
        reply_to: None,
        header: None,
        payload,
    }
}

/// Encodes a MSG into a wire frame ready to send.
#[allow(dead_code)]
pub fn encoded_msg(msg: Msg) -> Result<Bytes, EncodeError> {
    ServerCodec.encode(&Message::Msg(msg))
}

/// Generates a self-signed certificate for localhost (dev only).
fn dev_certificate()
-> Result<(Vec<rustls_pki_types::CertificateDer<'static>>, PrivateKeyDer<'static>)> {
    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(["localhost".into()])?;
    let key_der = signing_key.serialize_der();
    let key_static: &'static [u8] = Box::leak(key_der.into_boxed_slice());
    let key = PrivateKeyDer::try_from(key_static).map_err(anyhow::Error::msg)?;
    let cert_der = rustls_pki_types::CertificateDer::from(cert.der().to_vec());
    Ok((vec![cert_der], key))
}

/// Binds the QUIC server to `addr` and returns the endpoint.
/// Uses a temporary self-signed cert; suitable for local/dev only.
pub async fn bind(addr: impl ToSocketAddrsAsync) -> Result<Endpoint> {
    let (cert_chain, key) = dev_certificate()?;
    let endpoint = ServerBuilder::new_with_single_cert(cert_chain, key)?
        .with_alpn_protocols(&[std::str::from_utf8(ALPN).unwrap()])
        .bind(addr)
        .await?;
    info!(local = %endpoint.local_addr()?, "QUIC server listening");
    Ok(endpoint)
}
