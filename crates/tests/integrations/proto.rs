use bytes::BytesMut;
use server::protocol::{ClientInbound, ClientOutbound};

use crate::utils::{
    client::{connect_client, recv, send},
    server::start_test_server,
};

#[tokio::test]
async fn test_protocol_basic() {
    let server = start_test_server().await;
    let addr = server.addr();

    // 1. PING → PONG
    {
        let mut stream = connect_client(addr, server.cert_path()).await;

        send(&mut stream, &ClientOutbound::Ping).await;
        let response = recv(&mut stream).await;

        assert_eq!(response, ClientInbound::Pong, "server should reply PONG to a PING");
    }

    // 2. SUB → Ok
    {
        let mut stream = connect_client(addr, server.cert_path()).await;

        send(
            &mut stream,
            &ClientOutbound::Sub {
                topic: BytesMut::from("test/topic"),
                subscription_id: bytes::Bytes::from_static(b"sub-1"),
                queue_group: None,
            },
        )
        .await;

        let response = recv(&mut stream).await;

        assert_eq!(response, ClientInbound::Ok, "server should acknowledge SUB with Ok");
    }

    // 3 & 4. PUB + MSG delivery
    {
        let mut sub_stream = connect_client(addr, server.cert_path()).await;

        // Subscribe first
        send(
            &mut sub_stream,
            &ClientOutbound::Sub {
                topic: BytesMut::from("chat/room1"),
                subscription_id: bytes::Bytes::from_static(b"sub-room1"),
                queue_group: None,
            },
        )
        .await;

        // Consume the Ok acknowledgement for SUB
        let sub_ack = recv(&mut sub_stream).await;
        assert_eq!(
            sub_ack,
            ClientInbound::Ok,
            "server should acknowledge SUB with Ok before MSG delivery"
        );

        // Publish from a separate stream / connection
        let mut pub_stream = connect_client(addr, server.cert_path()).await;

        send(
            &mut pub_stream,
            &ClientOutbound::Pub {
                topic: BytesMut::from("chat/room1"),
                header: None,
                payload: bytes::Bytes::from_static(b"hello world"),
            },
        )
        .await;

        // The subscriber should receive a MSG frame
        let msg = recv(&mut sub_stream).await;

        match msg {
            ClientInbound::Msg { topic, subscription_id, payload, .. } => {
                assert_eq!(
                    topic,
                    BytesMut::from("chat/room1"),
                    "MSG topic should match the published topic"
                );
                assert_eq!(
                    subscription_id,
                    bytes::Bytes::from_static(b"sub-room1"),
                    "MSG subscription_id should match the subscriber's id"
                );
                assert_eq!(
                    payload,
                    bytes::Bytes::from_static(b"hello world"),
                    "MSG payload should match the published payload"
                );
            }
            other => {
                panic!("expected ClientInbound::Msg, but got: {other:?}");
            }
        }
    }

    server.shutdown().await;
}
