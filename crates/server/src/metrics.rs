use std::sync::{LazyLock, OnceLock};

use axum::{Router, body::Body, response::Response, routing::get};
use prometheus::{
    Encoder, IntCounter, IntGauge, TextEncoder, register_int_counter, register_int_gauge,
};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

pub struct MetricsManager;

impl MetricsManager {
    pub fn boot_metrics_service(listen_addr: String, shutdown: CancellationToken) {
        static METRICS_SERVICE_LISTEN_ADDR: OnceLock<String> = OnceLock::new();
        let requested_addr = listen_addr.clone();

        let current_listen_addr = METRICS_SERVICE_LISTEN_ADDR.get_or_init(|| {
            let pc = prometheus::process_collector::ProcessCollector::for_self();
            let _ = prometheus::register(Box::new(pc));

            let spawn_addr = listen_addr.clone();

            info!("Prometheus listening to {}", spawn_addr);

            tokio::spawn(async move {
                let service = Router::new().route("/metrics", get(metrics)).into_make_service();
                let listener = TcpListener::bind(&spawn_addr).await.unwrap();

                let serve_future =
                    axum::serve(listener, service).with_graceful_shutdown(async move {
                        shutdown.cancelled().await;
                        info!("Prometheus service shutting down");
                    });

                if let Err(err) = serve_future.await {
                    error!(%err, "metrics service exited with error");
                }
            });

            listen_addr
        });

        if requested_addr != *current_listen_addr {
            warn!(
                "unable to listen port {} for metrics service. Currently listening on {}",
                requested_addr, current_listen_addr
            );
        }
    }
}

async fn metrics() -> Response<Body> {
    let mf = prometheus::gather();

    let encoder = TextEncoder::new();
    let mut buffer = Vec::new();
    encoder.encode(&mf, &mut buffer).unwrap();

    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, encoder.format_type())
        .body(Body::from(buffer))
        .unwrap()
}

#[allow(dead_code)]
pub static OCYPODE_ACTIVE_CONNECTIONS: LazyLock<IntGauge> = LazyLock::new(|| {
    register_int_gauge!("ocypode_active_connections", "Current number of active QUIC connections")
        .unwrap()
});

#[allow(dead_code)]
pub static OCYPODE_MESSAGES_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    register_int_counter!("ocypode_messages_total", "Total number of messages processed").unwrap()
});

#[allow(dead_code)]
pub static OCYPODE_ERRORS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    register_int_counter!("ocypode_errors_total", "Total number of errors occurred").unwrap()
});
