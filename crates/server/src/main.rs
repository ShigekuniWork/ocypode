use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
mod config;
mod core;
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

    tokio::select! {
        res = quic::server::start(&config) => {
            if let Err(e) = res {
                error!("Server error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl-C, shutting down…");
        }
    }

    // Stop profiler and write reports (flamegraph + pprof protobuf)
    profiler.stop_and_report();
    Ok(())
}
