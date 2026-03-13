use server::{config::GrpcConfig, grpc::grpc_serve};
use tokio_util::sync::CancellationToken;
use tonic_health::pb::{HealthCheckRequest, health_client::HealthClient};

#[tokio::test]
async fn test_grpc_health_check() {
    let config = GrpcConfig { listen_addr: "127.0.0.1:0".parse().unwrap() };

    let cancellation_token = CancellationToken::new();

    let listen_addr = grpc_serve(&config, cancellation_token).await;

    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", listen_addr))
        .unwrap()
        .connect()
        .await
        .expect("Failed to connect to server");

    let mut client = HealthClient::new(channel);

    let request = HealthCheckRequest { service: "ocypode-service".to_string() };

    let response = client.check(request).await.expect("Health check request failed").into_inner();

    assert_eq!(response.status as i32, 1);
}
