use std::{fs, net::SocketAddr};

use server::{
    config::{Config, QuicConfig, TlsConfig},
    quic::server::ServerHandle,
};

/// Build a [`Config`] suitable for integration tests.
///
/// Binds to `127.0.0.1:0` so the OS assigns a free port, avoiding conflicts
/// between parallel test runs.
fn test_config() -> (Config, tempfile::TempDir, std::path::PathBuf) {
    let cert =
        rcgen::generate_simple_self_signed(vec!["localhost".into(), "127.0.0.1".into()]).unwrap();
    let temp_dir = tempfile::tempdir().unwrap();
    let cert_path = temp_dir.path().join("server.crt");
    let key_path = temp_dir.path().join("key.pem");

    fs::write(&cert_path, cert.cert.pem()).unwrap();
    fs::write(&key_path, cert.signing_key.serialize_pem()).unwrap();

    let config = Config {
        quic: QuicConfig { endpoint: "127.0.0.1:0".to_string() },
        tls: TlsConfig {
            cert_path: cert_path.to_string_lossy().into_owned(),
            private_key: key_path.to_string_lossy().into_owned(),
        },
    };

    (config, temp_dir, cert_path)
}

/// Start the real server on an OS-assigned port and return a handle.
///
/// The returned [`TestServer`] exposes the bound address for clients to
/// connect to and a `shutdown()` method for clean teardown.
pub async fn start_test_server() -> TestServer {
    let (config, _temp_dir, cert_path) = test_config();
    let handle = server::quic::server::start(&config).await.expect("failed to start test server");

    TestServer { handle, _temp_dir, cert_path }
}

/// Thin wrapper around [`ServerHandle`] for ergonomic use in tests.
pub struct TestServer {
    handle: ServerHandle,
    _temp_dir: tempfile::TempDir,
    cert_path: std::path::PathBuf,
}

impl TestServer {
    /// The path to the temporary server certificate.
    pub fn cert_path(&self) -> &std::path::Path {
        &self.cert_path
    }

    /// The address the server is listening on.
    pub fn addr(&self) -> SocketAddr {
        self.handle.local_addr()
    }

    /// Gracefully shut down the server and wait for the task to finish.
    pub async fn shutdown(self) {
        self.handle.shutdown().await;
    }
}
