// TODO: This module owns the per-connection pipeline:
//       FramedRead → Handshake → Frame dispatch → Permission check → Router → FramedWrite.
//       Permission check (permission.rs) and routing (router.rs) are stubs pending implementation.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use futures_util::SinkExt;
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::mpsc,
};
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, FramedWrite};

static CLIENT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

use crate::{
    auth::Authenticator,
    config::ServerConfig,
    error::ServerCodecError,
    handshake::{CompletedHandshake, HandshakeError, PendingHandshake},
    parser::{Frame, OutboundMessage, PROTOCOL_VERSION, ServerCodec, ServerOutbound, pb},
    transport::Transport,
};

#[derive(Debug, Error)]
pub enum ClientError {
    #[error(transparent)]
    Handshake(#[from] HandshakeError),
    #[error(transparent)]
    Codec(#[from] ServerCodecError),
    #[error("outbound channel closed")]
    OutboundChannelClosed,
}

impl From<mpsc::error::SendError<OutboundMessage>> for ClientError {
    fn from(_: mpsc::error::SendError<OutboundMessage>) -> Self {
        ClientError::OutboundChannelClosed
    }
}

/// Server-side representation of a connected client.
/// Created immediately after the QUIC stream is accepted, before the handshake.
pub struct Client<R: AsyncRead + Unpin + Send> {
    client_id: u64,
    /// Read buffer (FramedRead holds a 32 KiB byte buffer internally).
    framed_read: FramedRead<R, ServerCodec>,
    /// Sender end of the outbound write-buffer channel.
    /// The writer task drains this channel and batch-flushes to the network.
    outbound_sender: mpsc::Sender<OutboundMessage>,
    authenticator: Arc<dyn Authenticator>,
    config: Arc<ServerConfig>,
}

impl<R: AsyncRead + Unpin + Send + 'static> Client<R> {
    /// Constructs a client from any Transport.
    /// Spawns an internal writer task that owns FramedWrite and the outbound channel receiver.
    pub fn new<T: Transport<Reader = R>>(
        transport: T,
        authenticator: Arc<dyn Authenticator>,
        config: Arc<ServerConfig>,
    ) -> Self {
        let client_id = CLIENT_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        let (reader, writer) = transport.into_split();
        let framed_read =
            FramedRead::with_capacity(reader, ServerCodec, config.quic.read_buffer_size);
        let framed_write =
            FramedWrite::with_capacity(writer, ServerCodec, config.quic.write_buffer_size);

        let (outbound_sender, outbound_receiver) =
            mpsc::channel(config.quic.outbound_channel_capacity);
        tokio::spawn(run_outbound_writer(framed_write, outbound_receiver));

        Self { client_id, framed_read, outbound_sender, authenticator, config }
    }

    /// Runs the full client pipeline: handshake then frame dispatch.
    pub async fn run(mut self) -> Result<(), ClientError> {
        // Build INFO once from ServerConfig before entering the handshake.
        let info = ServerOutbound::info(
            PROTOCOL_VERSION,
            self.client_id,
            self.config.server_id.clone(),
            self.config.server_name.clone(),
            self.config.requires_auth,
            self.config.tls_verify,
        );

        // Phase 1: Handshake
        let completed = perform_handshake(
            &mut self.framed_read,
            &self.outbound_sender,
            self.config.quic.connect_timeout,
            PendingHandshake::new(self.client_id),
            self.authenticator.as_ref(),
            info,
        )
        .await?;
        tracing::info!("client_id={} connection established", completed.client_id);

        // Phase 2: Frame dispatch loop (hot path)
        while let Some(frame) = self.framed_read.next().await {
            dispatch_frame(frame?, &completed, &self.outbound_sender)?;
        }

        Ok(())
    }
}

async fn perform_handshake<R: AsyncRead + Unpin>(
    framed_read: &mut FramedRead<R, ServerCodec>,
    outbound: &mpsc::Sender<OutboundMessage>,
    connect_timeout_ms: u64,
    pending: PendingHandshake,
    authenticator: &dyn Authenticator,
    info: pb::Info,
) -> Result<CompletedHandshake, ClientError> {
    use std::time::Duration;

    use tokio::time::timeout;

    outbound.send(OutboundMessage::Info(info)).await?;

    timeout(Duration::from_millis(connect_timeout_ms), async {
        match framed_read.next().await {
            Some(Ok(Frame::Connect(connect))) => {
                pending.on_connect(connect, authenticator).map_err(ClientError::Handshake)
            }
            // Publish/Subscribe/UnSubscribe before handshake completes is invalid.
            Some(Ok(_)) => Err(ClientError::Handshake(HandshakeError::ConnectionClosed)),
            Some(Err(e)) => Err(ClientError::Codec(e)),
            None => Err(ClientError::Handshake(HandshakeError::ConnectionClosed)),
        }
    })
    .await
    .map_err(|_| ClientError::Handshake(HandshakeError::ConnectTimeout))?
}

fn dispatch_frame(
    frame: Frame,
    handshake: &CompletedHandshake,
    _outbound: &mpsc::Sender<OutboundMessage>,
) -> Result<(), ClientError> {
    match frame {
        Frame::Connect(_) => {
            tracing::warn!(
                "client_id={} received unexpected CONNECT after handshake",
                handshake.client_id
            );
        }
        // TODO: permission check → router dispatch
        Frame::Publish(_) | Frame::Subscribe(_) | Frame::UnSubscribe(_) => {}
    }
    Ok(())
}

/// Drains the outbound channel and batch-flushes to FramedWrite.
/// Minimizes syscall overhead by coalescing multiple messages into a single flush.
async fn run_outbound_writer<W: AsyncWrite + Unpin>(
    mut framed_write: FramedWrite<W, ServerCodec>,
    mut receiver: mpsc::Receiver<OutboundMessage>,
) {
    while let Some(message) = receiver.recv().await {
        let _ = dispatch_outbound(&mut framed_write, message).await;

        // Non-blocking drain: feed all queued messages before flushing.
        while let Ok(message) = receiver.try_recv() {
            let _ = dispatch_outbound(&mut framed_write, message).await;
        }

        // One flush per batch → minimizes syscalls.
        // The type annotation resolves ambiguity: ServerCodec encodes multiple item types.
        let _ = SinkExt::<pb::Info>::flush(&mut framed_write).await;
    }
}

async fn dispatch_outbound<W: AsyncWrite + Unpin>(
    framed_write: &mut FramedWrite<W, ServerCodec>,
    message: OutboundMessage,
) -> Result<(), ServerCodecError> {
    match message {
        OutboundMessage::Info(info) => framed_write.feed(info).await?,
        // TODO: Message delivery to subscribers
        OutboundMessage::Message(_) => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use futures_util::SinkExt;
    use tokio::io::{AsyncRead, AsyncWrite};
    use tokio_stream::StreamExt;
    use tokio_util::codec::{FramedRead, FramedWrite};

    use super::Client;
    use crate::{
        auth::NoAuthAuthenticator,
        config::ServerConfig,
        parser::{ClientCodec, ClientFrame, ClientOutbound},
        transport::Transport,
    };

    struct DuplexTransport<R, W> {
        reader: R,
        writer: W,
    }

    impl<R, W> Transport for DuplexTransport<R, W>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        type Reader = R;
        type Writer = W;

        fn into_split(self) -> (R, W) {
            (self.reader, self.writer)
        }
    }

    #[tokio::test]
    async fn client_run_sends_info_and_accepts_connect() {
        let (client_io, server_io) = tokio::io::duplex(4096);
        let (server_rx, server_tx) = tokio::io::split(server_io);
        let (client_rx, client_tx) = tokio::io::split(client_io);

        let transport = DuplexTransport { reader: server_rx, writer: server_tx };
        let client =
            Client::new(transport, Arc::new(NoAuthAuthenticator), Arc::new(ServerConfig::new()));
        let server = tokio::spawn(client.run());

        // Act as a network client: read INFO, send CONNECT.
        let mut framed_read = FramedRead::with_capacity(client_rx, ClientCodec, 4096);
        let frame = framed_read.next().await.unwrap().unwrap();
        let ClientFrame::Info(info_msg) = frame else { panic!("expected Info frame") };
        assert_eq!(info_msg.client_id, 1);

        let mut framed_write = FramedWrite::with_capacity(client_tx, ClientCodec, 4096);
        framed_write.send(ClientOutbound::connect(1, false)).await.unwrap();

        // Drop the write end to signal EOF → server run() should finish cleanly.
        drop(framed_write);
        drop(framed_read);

        server.await.unwrap().unwrap();
    }
}
