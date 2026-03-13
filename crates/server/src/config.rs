use std::net::SocketAddr;

use tracing::level_filters::LevelFilter;

/// Ocypode server configuration.
pub struct ServerConfig {
    pub logger: LoggerConfig,
    pub grpc: GrpcConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerConfig {
    // TODO: should load config from file.
    pub fn new() -> Self {
        Self { logger: LoggerConfig::default(), grpc: GrpcConfig::default() }
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
            enable_tokio_console: false,
            with_thread_name: false,
            default_level: LevelFilter::INFO,
        }
    }
}

pub struct GrpcConfig {
    pub listen_addr: String,
}

impl Default for GrpcConfig {
    fn default() -> Self {
        Self { listen_addr: "[::1]:50051".to_string() }
    }
}

impl GrpcConfig {
    pub fn socket_addr(&self) -> SocketAddr {
        self.listen_addr.parse().unwrap()
    }
}
