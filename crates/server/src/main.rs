use tracing::info;

use crate::{config::ServerConfig, logger::init_ocypode_logger};

mod config;
mod logger;

fn main() {
    let config = ServerConfig::new();
    init_ocypode_logger(&config.logger);

    info!("Starting ocypode-server");
    info!("Server is ready");
}
