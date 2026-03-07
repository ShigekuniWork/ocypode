use std::{error::Error, net::SocketAddr, path::Path, sync::Arc};

use anyhow::Result;
use bytes::{Bytes, BytesMut};
use s2n_quic::Server;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use tracing::{debug, info, warn};

use crate::{
    config::Config,
    protocol::{ProtocolCodec, ServerCodec, ServerInbound, ServerOutbound},
    routing::Router,
    subscriber::Subscriber,
    topic::{Topic, TopicKind},
};

/// Handle to a running server.
///
/// Exposes the bound address and provides graceful shutdown.
pub struct ServerHandle {
    addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
    handle: Option<JoinHandle<()>>,
}

impl ServerHandle {
    /// The address the server is actually listening on.
    ///
    /// Useful when binding to port `0` to discover the OS-assigned port.
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Send the shutdown signal and wait for the server task to finish.
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.await;
        }
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        // Best-effort shutdown if `shutdown()` was not called explicitly.
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// Start the QUIC server described by `config`.
///
/// Returns a [`ServerHandle`] immediately after the server socket is bound.
/// The accept loop runs in a background task and can be stopped via
/// [`ServerHandle::shutdown`].
pub async fn start(config: &Config) -> Result<ServerHandle, Box<dyn Error>> {
    let tls = &config.tls;

    let mut server = Server::builder()
        .with_tls((Path::new(&tls.cert_path), Path::new(&tls.private_key)))?
        .with_io(config.quic.endpoint.as_str())?
        .start()?;

    let addr = server.local_addr()?;

    info!("Using cert: {}", tls.cert_path);
    info!("Using key:  {}", tls.private_key);
    info!("Listening on: {}", addr);

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let handle = tokio::spawn(async move {
        tokio::select! {
            _ = accept_loop(&mut server) => {}
            _ = shutdown_rx => {
                info!("Shutdown signal received");
            }
        }
    });

    Ok(ServerHandle { addr, shutdown_tx: Some(shutdown_tx), handle: Some(handle) })
}

async fn accept_loop(server: &mut Server) {
    let router = Arc::new(Router::new());

    while let Some(mut connection) = server.accept().await {
        let router = Arc::clone(&router);

        tokio::spawn(async move {
            let conn_id = connection.id().to_string();

            while let Ok(Some(mut stream)) = connection.accept_bidirectional_stream().await {
                let router = Arc::clone(&router);
                let conn_id = conn_id.clone();

                tokio::spawn(async move {
                    let mut buf = BytesMut::new();
                    // Per-stream outbound channel: the router sends already-encoded
                    // Bytes here, which are forwarded to the peer stream.
                    let (tx, mut rx) = mpsc::channel::<Bytes>(64);

                    loop {
                        tokio::select! {
                            // Incoming bytes from the peer
                            incoming = stream.receive() => {
                                match incoming {
                                    Ok(Some(data)) => {
                                        buf.extend_from_slice(&data);

                                        match ServerCodec::decode(&mut buf) {
                                            Ok(msg) => {
                                                debug!("decoded: {:?}", msg);
                                                if let Some(resp) = handle_inbound(&router, &conn_id, tx.clone(), msg) {
                                                    let encoded = ServerCodec::encode(&resp);
                                                    let _ = stream.send(encoded.freeze()).await;
                                                }
                                            }
                                            Err(e) => {
                                                warn!("failed to decode message: {e}");
                                            }
                                        }
                                    }
                                    Ok(None) => break,
                                    Err(e) => {
                                        warn!("stream receive error: {e}");
                                        break;
                                    }
                                }
                            }

                            // Outbound frames queued by the router for this stream
                            maybe_out = rx.recv() => {
                                match maybe_out {
                                    Some(out_bytes) => {
                                        let _ = stream.send(out_bytes).await;
                                    }
                                    None => {
                                        // All senders dropped — keep serving incoming data.
                                    }
                                }
                            }
                        }
                    }

                    // TODO: track subscribed topics per connection and unsubscribe on disconnect
                });
            }
        });
    }
}

fn handle_inbound(
    router: &Router,
    conn_id: &str,
    tx: mpsc::Sender<Bytes>,
    msg: ServerInbound,
) -> Option<ServerOutbound> {
    match msg {
        ServerInbound::Connect { version, .. } => {
            debug!("client connected with protocol version {version}");
            None
        }
        ServerInbound::Pub { topic, payload, header } => {
            match Topic::parse(topic.freeze(), TopicKind::Publish) {
                Ok(topic) => {
                    router.publish(&topic, |subscriber| {
                        // Extract the subscription_id portion from "{conn_id}/{sub_id}".
                        let sub_id = subscriber
                            .id()
                            .rsplit('/')
                            .next()
                            .unwrap_or(subscriber.id())
                            .to_string();

                        let msg = ServerOutbound::Msg {
                            topic: BytesMut::from(topic.as_bytes()),
                            subscription_id: Bytes::from(sub_id),
                            header: header.clone(),
                            payload: payload.clone(),
                        };
                        ServerCodec::encode(&msg).freeze()
                    });
                }
                Err(e) => {
                    warn!("invalid publish topic: {e}");
                    // TODO: send Err(InvalidTopic) back to the client
                }
            }
            None
        }
        ServerInbound::Sub { topic, subscription_id, .. } => {
            match Topic::parse(topic.freeze(), TopicKind::Subscribe) {
                Ok(topic) => {
                    let sub_id = String::from_utf8_lossy(&subscription_id).into_owned();
                    let id = format!("{conn_id}/{sub_id}");
                    let subscriber = Subscriber::new(id, tx);
                    router.subscribe(topic, subscriber);
                    Some(ServerOutbound::Ok)
                }
                Err(e) => {
                    warn!("invalid subscribe topic: {e}");
                    // TODO: send Err(InvalidTopic) back to the client
                    None
                }
            }
        }
        ServerInbound::Unsub { subscription_id } => {
            let sub_id = String::from_utf8_lossy(&subscription_id).into_owned();
            let id = format!("{conn_id}/{sub_id}");
            // TODO: track topic per subscription_id; using a placeholder topic for now
            let placeholder = Topic::new(bytes::Bytes::from_static(b""));
            router.un_subscribe(&id, &placeholder);
            None
        }
        ServerInbound::Ping => Some(ServerOutbound::Pong),
        ServerInbound::Pong => None,
    }
}
