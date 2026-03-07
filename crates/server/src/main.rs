use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
mod config;
mod subscriber;
mod topic;
use config::Config;
use tracing::{error, info};

use crate::{log::Logging, profiler::Profiler};

mod log;
mod profiler;
mod protocol;
mod quic;
mod routing;

#[derive(Debug, Parser)]
#[command(about = "Ocypode QUIC server")]
struct Args {
    /// Path to the configuration YAML file
    #[arg(short = 'c', long)]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let logging = Logging::new();
    logging.early_init();

    // Start profiler (no-op when `profiling` feature is disabled)
    let profiler = Profiler::start();

    let args = Args::parse();

    info!("Welcome to Ocypode");

    // Load config
    let config = Config::load_from_path(&args.config).unwrap_or_else(|e| panic!("{}", e));

    let server = quic::server::start(&config).await.unwrap_or_else(|e| {
        error!("Failed to start server: {}", e);
        std::process::exit(1);
    });

    info!("Server listening on {}", server.local_addr());

    tokio::signal::ctrl_c().await.ok();
    info!("Received Ctrl-C, shutting down…");

    server.shutdown().await;

    // Stop profiler and write reports (flamegraph + pprof protobuf)
    profiler.stop_and_report();
    Ok(())
}
