use tracing::info;

use crate::log::Logging;

mod log;

#[tokio::main]
async fn main() {
    // Initialize logging
    let logging = Logging::new();
    logging.early_init();

    info!("Welcome to Ocypode");
}
