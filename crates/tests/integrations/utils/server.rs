use std::{net::SocketAddr, path::PathBuf};

use server::{
    config::{Config, QuicConfig, TlsConfig},
    quic::server::ServerHandle,
};

/// Returns the path to the test certs directory (`crates/tests/certs/`).
fn certs_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("certs")
}

/// Build a [`Config`] suitable for integration tests.
///
/// Binds to `127.0.0.1:0` so the OS assigns a free port, avoiding conflicts
/// between parallel test runs.
fn test_config() -> Config {
    let cert_path = certs_dir().join("server.crt");
    let key_path = certs_dir().join("key.pem");

    assert!(cert_path.exists(), "Missing test cert: {}", cert_path.display());
    assert!(key_path.exists(), "Missing test key: {}", key_path.display());

    Config {
        quic: QuicConfig { endpoint: "127.0.0.1:0".to_string() },
        tls: TlsConfig {
            cert_path: cert_path.to_string_lossy().into_owned(),
            private_key: key_path.to_string_lossy().into_owned(),
        },
    }
}

/// Start the real server on an OS-assigned port and return a handle.
///
/// The returned [`TestServer`] exposes the bound address for clients to
/// connect to and a `shutdown()` method for clean teardown.
pub async fn start_test_server() -> TestServer {
    let config = test_config();
    let handle = server::quic::server::start(&config).await.expect("failed to start test server");

    TestServer { handle }
}

/// Thin wrapper around [`ServerHandle`] for ergonomic use in tests.
pub struct TestServer {
    handle: ServerHandle,
}

impl TestServer {
    /// The address the server is listening on.
    pub fn addr(&self) -> SocketAddr {
        self.handle.local_addr()
    }

    /// Gracefully shut down the server and wait for the task to finish.
    pub async fn shutdown(self) {
        self.handle.shutdown().await;
    }
}
