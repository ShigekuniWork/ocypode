use std::{error::Error, path::PathBuf};

use clap::Parser;
use s2n_quic::Server;
mod config;
use config::Config;
use tracing::info;

use crate::{log::Logging, profiler::Profiler};

mod log;
mod profiler;

#[derive(Debug, Parser)]
#[command(about = "Ocypode QUIC server")]
struct Args {
    /// Path to the configuration YAML file
    #[arg(short = 'c', long)]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    let logging = Logging::new();
    logging.early_init();

    // Start profiler (no-op when `profiling` feature is disabled)
    let profiler = Profiler::start();

    let args = Args::parse();

    info!("Welcome to Ocypode");

    // Load config
    let config = Config::load_from_path(&args.config).unwrap_or_else(|e| panic!("{}", e));

    let tls = &config.quic.tls;
    let cert = std::fs::read_to_string(&tls.cert_path)
        .unwrap_or_else(|e| panic!("Failed to read cert file {}: {e}", tls.cert_path));
    let key = std::fs::read_to_string(&tls.private_key)
        .unwrap_or_else(|e| panic!("Failed to read key file {}: {e}", tls.private_key));

    let mut server = Server::builder()
        .with_tls((cert.as_str(), key.as_str()))?
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

    // Wait for Ctrl-C to gracefully shut down
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            info!("Received Ctrl-C, shutting down…");
        }
        Err(e) => {
            tracing::error!("Failed to listen for shutdown signal: {e}");
        }
    }

    // Stop profiler and write reports (flamegraph + pprof protobuf)
    profiler.stop_and_report();
    Ok(())
}
