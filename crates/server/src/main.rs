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
    Runtime::new()?.block_on(quic::serve(QUIC_LISTEN))
}
