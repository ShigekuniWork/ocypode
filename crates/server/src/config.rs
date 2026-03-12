use tracing::level_filters::LevelFilter;

/// Ocypode server configuration.
pub struct ServerConfig {
    pub logger: LoggerConfig,
}

impl ServerConfig {
    // TODO: should load config from file.
    pub fn new() -> Self {
        Self { logger: LoggerConfig::default() }
    }
}

pub struct LoggerConfig {
    pub name: String,
    pub enable_tokio_console: bool,
    pub with_thread_name: bool,
    pub default_level: LevelFilter,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self::new("ocypode")
    }
}

impl LoggerConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            enable_tokio_console: true,
            with_thread_name: false,
            default_level: LevelFilter::INFO,
        }
    }
}
