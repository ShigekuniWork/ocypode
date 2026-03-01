use tracing_subscriber::{EnvFilter, fmt, prelude::*};

const DEFAULT_LEVEL: &str = "info";

/// Logging configuration and initialization.
pub struct Logging;

impl Logging {
    pub fn new() -> Self {
        Self
    }

    /// Initializes the tracing subscriber (fmt + EnvFilter).
    /// Log level is controlled by `RUST_LOG`; if unset or invalid, defaults to INFO.
    pub fn early_init(self) {
        let filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LEVEL));
        tracing_subscriber::registry().with(fmt::layer()).with(filter).init();
    }
}
