use std::{
    io::{Error, ErrorKind},
    net::SocketAddr,
    path::Path,
};

use tracing::level_filters::LevelFilter;

/// Ocypode server configuration.
pub struct ServerConfig {
    pub logger: LoggerConfig,
    pub grpc: GrpcConfig,
    pub metrics: MetricsConfig,
    pub quic: QuicConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerConfig {
    // TODO: should load config from file.
    pub fn new() -> Self {
        Self {
            logger: LoggerConfig::default(),
            grpc: GrpcConfig::default(),
            metrics: MetricsConfig::default(),
            quic: QuicConfig::default(),
        }
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

pub struct MetricsConfig {
    pub metrics_level: MetricLevel,
    pub listen_addr: String,
}

#[allow(dead_code)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum MetricLevel {
    #[default]
    Disabled = 0,
    Critical = 1,
    Info = 2,
    Debug = 3,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self { metrics_level: MetricLevel::default(), listen_addr: "127.0.0.1:9090".to_string() }
    }
}

pub struct QuicConfig {
    pub listen_addr: String,
    pub enable_gso: bool,
    pub enable_gro: bool,
    pub endpoint_limits: Option<usize>,
    // QUIC requires TLS to be enabled.
    pub tls: TLSConfig,
}

impl Default for QuicConfig {
    fn default() -> Self {
        QuicConfig {
            listen_addr: "127.0.0.1:4433".to_string(),
            enable_gso: true,
            enable_gro: true,
            endpoint_limits: None,
            tls: TLSConfig::default(),
        }
    }
}

impl QuicConfig {
    pub fn socket_addr(&self) -> SocketAddr {
        self.listen_addr.parse().unwrap()
    }
}

pub struct TLSConfig {
    cert_file_path: String,
    key_file_path: String,
}

impl Default for TLSConfig {
    fn default() -> Self {
        // TODO: load from configuration file
        TLSConfig {
            cert_file_path: "crates/certs/server.crt".to_string(),
            key_file_path: "crates/certs/key.pem".to_string(),
        }
    }
}

impl TLSConfig {
    pub fn cert_file_path(&self) -> Result<&Path, Error> {
        let path = Path::new(&self.cert_file_path);
        if !path.try_exists()? {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("Certificate file not found at: {}", self.cert_file_path),
            ));
        }
        Ok(path)
    }

    pub fn key_file_path(&self) -> Result<&Path, Error> {
        let path = Path::new(&self.key_file_path);
        if !path.try_exists()? {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("Key file not found at: {}", self.key_file_path),
            ));
        }
        Ok(path)
    }
}
