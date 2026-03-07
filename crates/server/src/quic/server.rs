use std::{error::Error, path::Path, sync::Arc};

use anyhow::Result;
use bytes::{Bytes, BytesMut};
use s2n_quic::Server;
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::{
    config::Config,
    core::{Subscriber, Topic},
    protocol::{ProtocolCodec, ServerCodec, ServerInbound},
    routing::Router,
};

pub async fn start(config: &Config) -> Result<(), Box<dyn Error>> {
    let tls = &config.quic.tls;

    let mut server = Server::builder()
        .with_tls((Path::new(&tls.cert_path), Path::new(&tls.private_key)))?
        .with_io(config.quic.endpoint.as_str())?
        .start()?;

    info!("Using cert: {}", tls.cert_path);
    info!("Using key:  {}", tls.private_key);
    info!("Listening on: {}", config.quic.endpoint);

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
                    // TODO: replace with a proper per-connection outbound channel
                    let (tx, _rx) = mpsc::channel::<Bytes>(64);

                    while let Ok(Some(data)) = stream.receive().await {
                        buf.extend_from_slice(&data);

                        match ServerCodec::decode(&mut buf) {
                            Ok(msg) => {
                                debug!("{:?}", msg);
                                handle_inbound(&router, &conn_id, tx.clone(), msg);
                            }
                            Err(e) => {
                                tracing::warn!("failed to decode message: {e}");
                            }
                        }
                    }

                    // Clean up all subscriptions for this connection on disconnect
                    // TODO: track subscribed topics per connection and unsubscribe each
                });
            }
        });
    }

    Ok(())
}

fn handle_inbound(router: &Router, conn_id: &str, tx: mpsc::Sender<Bytes>, msg: ServerInbound) {
    match msg {
        ServerInbound::Sub { topic, subscription_id, .. } => {
            let topic = Topic::new(topic.freeze());
            let sub_id = String::from_utf8_lossy(&subscription_id).into_owned();
            let id = format!("{conn_id}/{sub_id}");
            let subscriber = Subscriber::new(id, tx);
            router.subscribe(topic, subscriber);
        }
        ServerInbound::Unsub { subscription_id } => {
            let sub_id = String::from_utf8_lossy(&subscription_id).into_owned();
            let id = format!("{conn_id}/{sub_id}");
            // TODO: track topic per subscription_id; using a placeholder topic for now
            let placeholder = Topic::new(bytes::Bytes::from_static(b""));
            router.un_subscribe(&id, &placeholder);
        }
        ServerInbound::Pub { topic, payload, .. } => {
            let topic = Topic::new(topic.freeze());
            router.publish(&topic, payload);
        }
        ServerInbound::Connect { version, .. } => {
            debug!("client connected with protocol version {version}");
        }
        ServerInbound::Ping | ServerInbound::Pong => {}
    }
}
