use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct QuicConfig {
    /// Socket address the server should bind to, e.g. "127.0.0.1:4433".
    pub endpoint: String,
}

#[derive(Debug, Deserialize)]
pub struct TlsConfig {
    /// Filesystem path to the PEM-encoded server certificate.
    pub cert_path: String,

    /// Filesystem path to the PEM-encoded private key.
    pub private_key: String,
}
