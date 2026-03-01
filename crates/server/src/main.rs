mod log;
mod quic;

use anyhow::Result;
use compio::runtime::Runtime;
use log::Logging;
use tracing::{info, instrument};

const QUIC_LISTEN: &str = "0.0.0.0:4433";

#[instrument(skip_all, name = "trace_start_server")]
fn main() -> Result<()> {
    let logging = Logging::new();
    logging.early_init();

    info!("server starting");
    Runtime::new()?.block_on(serve())
}

async fn serve() -> Result<()> {
    let endpoint = quic::bind(QUIC_LISTEN).await?;
    while let Some(incoming) = endpoint.wait_incoming().await {
        let connection = incoming.await?;
        info!(remote = ?connection.remote_address(), "QUIC connection accepted");
    }
    Ok(())
}
