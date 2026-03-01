mod log;

use log::Logging;
use tracing::{info, instrument};

#[instrument(skip_all, name = "trace_start_server")]
fn main() {
    let logging = Logging::new();
    logging.early_init();

    info!("server starting");
}
