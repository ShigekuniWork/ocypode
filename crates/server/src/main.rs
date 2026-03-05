use tracing::info;

use crate::{log::Logging, profiler::Profiler};

mod log;
mod profiler;

#[tokio::main]
async fn main() {
    // Initialize logging
    let logging = Logging::new();
    logging.early_init();

    // Start profiler (no-op when `profiling` feature is disabled)
    let profiler = Profiler::start();

    info!("Welcome to Ocypode");

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
}
