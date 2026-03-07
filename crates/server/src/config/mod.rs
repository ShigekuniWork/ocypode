use std::{fs, path::Path};

use serde::Deserialize;

mod quic;
pub use quic::{QuicConfig, TlsConfig};

/// Top-level configuration and loader.
#[derive(Debug, Deserialize)]
pub struct Config {
    pub quic: QuicConfig,
    pub tls: TlsConfig,
}

impl Config {
    /// Load a `Config` from the given filesystem path.
    ///
    /// Returns `Ok(Config)` on success or an `Err(String)` describing the
    /// failure (I/O or parse error).
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let path_ref = path.as_ref();
        let contents = fs::read_to_string(path_ref)
            .map_err(|e| format!("Failed to read config file {}: {}", path_ref.display(), e))?;
        serde_yaml::from_str(&contents)
            .map_err(|e| format!("Failed to parse config file {}: {}", path_ref.display(), e))
    }
}
