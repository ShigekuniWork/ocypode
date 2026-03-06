use std::{error::Error, path::Path};

use anyhow::Result;
use s2n_quic::Server;
use tracing::info;

use crate::config::Config;

pub async fn start(config: &Config) -> Result<(), Box<dyn Error>> {
    let tls = &config.quic.tls;

    let mut server = Server::builder()
        .with_tls((Path::new(&tls.cert_path), Path::new(&tls.private_key)))?
        .with_io(config.quic.endpoint.as_str())?
        .start()?;

    info!("Using cert: {}", tls.cert_path);
    info!("Using key:  {}", tls.private_key);
    info!("Listening on: {}", config.quic.endpoint);

    while let Some(mut connection) = server.accept().await {
        tokio::spawn(async move {
            while let Ok(Some(mut stream)) = connection.accept_bidirectional_stream().await {
                tokio::spawn(async move {
                    while let Ok(Some(data)) = stream.receive().await {
                        stream.send(data).await.expect("stream should be open");
                    }
                });
            }
        });
    }

    Ok(())
}
