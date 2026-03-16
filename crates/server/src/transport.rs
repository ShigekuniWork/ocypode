use tokio::io::{AsyncRead, AsyncWrite};

/// Abstracts a bidirectional byte stream transport.
/// Implementations exist for QUIC (via s2n-quic) and can be added for TCP or WebSocket.
pub trait Transport: Send + 'static {
    type Reader: AsyncRead + Unpin + Send + 'static;
    type Writer: AsyncWrite + Unpin + Send + 'static;

    fn into_split(self) -> (Self::Reader, Self::Writer);
}
