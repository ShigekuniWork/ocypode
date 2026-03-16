use bytes::{Buf, BufMut, Bytes, BytesMut};
use prost::Message;
use tokio_util::codec::{Decoder, Encoder};

use crate::error::{ClientCodecError, CodecError, ServerCodecError};
pub mod pb {
    include!(concat!(env!("OUT_DIR"), "/ocypode.pubsub.v1.rs"));
}

const COMMAND_BYTE_LEN: usize = 1;
const PAYLOAD_LENGTH_BYTES: usize = 4;
const HEADER_LENGTH: usize = COMMAND_BYTE_LEN + PAYLOAD_LENGTH_BYTES;
// Maximum payload is 1MiB.
pub const MAXIMUM_PAYLOAD_BYTES: usize = 1024 * 1024;
/// Current Ocypode protocol version.
pub const PROTOCOL_VERSION: u32 = 1;

/// Command classify Ocypode protocol.
#[repr(u8)]
pub enum Command {
    Info = 0x00,
    Connect = 0x01,
    Publish = 0x02,
    Subscribe = 0x03,
    UnSubscribe = 0x04,
    Message = 0x05,
    // TODO: add Err command.
}

/// Command trait for payload encode/decode.
pub trait CommandCodec: Message + Default + Sized {
    const COMMAND: u8;

    fn encode_payload(&self) -> Result<Bytes, CodecError> {
        let mut payload_buffer = Vec::with_capacity(self.encoded_len());
        self.encode(&mut payload_buffer)?;
        Ok(Bytes::from(payload_buffer))
    }

    fn decode_payload(payload: &[u8]) -> Result<Self, CodecError> {
        Ok(Self::decode(payload)?)
    }
}

impl CommandCodec for pb::Info {
    const COMMAND: u8 = Command::Info as u8;
}

impl CommandCodec for pb::Connect {
    const COMMAND: u8 = Command::Connect as u8;
}

impl CommandCodec for pb::Publish {
    const COMMAND: u8 = Command::Publish as u8;
}

impl CommandCodec for pb::Subscribe {
    const COMMAND: u8 = Command::Subscribe as u8;
}

impl CommandCodec for pb::UnSubscribe {
    const COMMAND: u8 = Command::UnSubscribe as u8;
}

impl CommandCodec for pb::Message {
    const COMMAND: u8 = Command::Message as u8;
}

#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    Connect(pb::Connect),
    Publish(pb::Publish),
    Subscribe(pb::Subscribe),
    UnSubscribe(pb::UnSubscribe),
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum ClientFrame {
    Info(pb::Info),
    Message(pb::Message),
}

/// Messages the server sends to a connected client.
/// Used as the element type for the outbound write-buffer channel.
#[allow(dead_code)]
pub enum OutboundMessage {
    Info(pb::Info),
    Message(pb::Message),
    // TODO: Pong, Error(pb::Error), etc.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerInboundCommand {
    Connect,
    Publish,
    Subscribe,
    UnSubscribe,
}

impl TryFrom<u8> for ServerInboundCommand {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            _ if value == <pb::Connect as CommandCodec>::COMMAND => Ok(ServerInboundCommand::Connect),
            _ if value == <pb::Publish as CommandCodec>::COMMAND => Ok(ServerInboundCommand::Publish),
            _ if value == <pb::Subscribe as CommandCodec>::COMMAND => Ok(ServerInboundCommand::Subscribe),
            _ if value == <pb::UnSubscribe as CommandCodec>::COMMAND => Ok(ServerInboundCommand::UnSubscribe),
            _ => Err(()),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientInboundCommand {
    Info,
    Message,
}

impl TryFrom<u8> for ClientInboundCommand {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            _ if value == <pb::Info as CommandCodec>::COMMAND => Ok(ClientInboundCommand::Info),
            _ if value == <pb::Message as CommandCodec>::COMMAND => Ok(ClientInboundCommand::Message),
            _ => Err(()),
        }
    }
}

/// Server outbound message builder
pub struct ServerOutbound;

impl ServerOutbound {
    /// Creates an INFO message with specified parameters
    pub fn info(
        version: u32,
        client_id: u64,
        server_id: String,
        server_name: String,
        requires_auth: bool,
        tls_verify: bool,
    ) -> pb::Info {
        pb::Info {
            version,
            server_id,
            server_name,
            max_payload: MAXIMUM_PAYLOAD_BYTES as u32,
            client_id,
            requires_auth,
            tls_verify,
        }
    }

    /// Creates a default INFO message
    /// TODO: Load INFO message from configuration instead of using dummy values
    #[allow(dead_code)]
    pub fn default_info() -> pb::Info {
        Self::info(1, 0, "ocypode-server".to_string(), "ocypode".to_string(), false, false)
    }
}

/// Client outbound message builder
#[allow(dead_code)]
pub struct ClientOutbound;

impl ClientOutbound {
    /// Creates a CONNECT message with specified parameters
    #[allow(dead_code)]
    pub fn connect(version: u32, verbose: bool) -> pb::Connect {
        pb::Connect {
            version,
            verbose,
            auth_method: pb::AuthMethod::NoAuth as i32,
            credentials: None,
        }
    }

    /// Creates a CONNECT message with password credentials
    #[allow(dead_code)]
    pub fn connect_with_password(
        version: u32,
        verbose: bool,
        username: String,
        password: String,
    ) -> pb::Connect {
        pb::Connect {
            version,
            verbose,
            auth_method: pb::AuthMethod::Password as i32,
            credentials: Some(pb::connect::Credentials::PasswordAuth(pb::PasswordAuth {
                username,
                password,
            })),
        }
    }
}

fn parse_header(incoming_bytes: &BytesMut) -> Option<(u8, usize)> {
    if incoming_bytes.len() < HEADER_LENGTH {
        return None;
    }

    let mut header_bytes = &incoming_bytes[..HEADER_LENGTH];
    let command = header_bytes.get_u8();
    let payload_length = header_bytes.get_u32() as usize;
    Some((command, payload_length))
}

pub struct ServerCodec;

impl Decoder for ServerCodec {
    type Item = Frame;
    type Error = ServerCodecError;

    fn decode(&mut self, incoming_bytes: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        loop {
            let Some((command, payload_length)) = parse_header(incoming_bytes) else {
                return Ok(None);
            };

            let command = match ServerInboundCommand::try_from(command) {
                Ok(value) => value,
                Err(()) => {
                    // Drop one byte to resync on an unexpected frame.
                    incoming_bytes.advance(1);
                    continue;
                }
            };

            if payload_length > MAXIMUM_PAYLOAD_BYTES {
                // Invalid length; drop one byte and try to recover.
                incoming_bytes.advance(1);
                continue;
            }

            let frame_length = HEADER_LENGTH + payload_length;
            if incoming_bytes.len() < frame_length {
                return Ok(None);
            }

            incoming_bytes.advance(HEADER_LENGTH);
            let payload_bytes = incoming_bytes.split_to(payload_length);
            let frame = match command {
                ServerInboundCommand::Connect => {
                    Frame::Connect(pb::Connect::decode_payload(&payload_bytes)?)
                }
                ServerInboundCommand::Publish => {
                    Frame::Publish(pb::Publish::decode_payload(&payload_bytes)?)
                }
                ServerInboundCommand::Subscribe => {
                    Frame::Subscribe(pb::Subscribe::decode_payload(&payload_bytes)?)
                }
                ServerInboundCommand::UnSubscribe => {
                    Frame::UnSubscribe(pb::UnSubscribe::decode_payload(&payload_bytes)?)
                }
            };
            return Ok(Some(frame));
        }
    }
}

impl<T> Encoder<T> for ServerCodec
where
    T: CommandCodec,
{
    type Error = ServerCodecError;

    fn encode(&mut self, item: T, output_buffer: &mut BytesMut) -> Result<(), Self::Error> {
        let payload = item.encode_payload()?;
        let payload_length: u32 =
            payload.len().try_into().map_err(|_| CodecError::InvalidSizeBytes(payload.len()))?;

        output_buffer.reserve(HEADER_LENGTH + payload.len());
        output_buffer.put_u8(T::COMMAND);
        output_buffer.put_u32(payload_length);
        output_buffer.extend_from_slice(&payload);
        Ok(())
    }
}

#[allow(dead_code)]
pub struct ClientCodec;

impl Decoder for ClientCodec {
    type Item = ClientFrame;
    type Error = ClientCodecError;

    fn decode(&mut self, incoming_bytes: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        loop {
            let Some((command, payload_length)) = parse_header(incoming_bytes) else {
                return Ok(None);
            };

            let command = match ClientInboundCommand::try_from(command) {
                Ok(value) => value,
                Err(()) => {
                    // Drop one byte to resync on an unexpected frame.
                    incoming_bytes.advance(1);
                    continue;
                }
            };

            if payload_length > MAXIMUM_PAYLOAD_BYTES {
                // Invalid length; drop one byte and try to recover.
                incoming_bytes.advance(1);
                continue;
            }

            let frame_length = HEADER_LENGTH + payload_length;
            if incoming_bytes.len() < frame_length {
                return Ok(None);
            }

            incoming_bytes.advance(HEADER_LENGTH);
            let payload_bytes = incoming_bytes.split_to(payload_length);
            let frame = match command {
                ClientInboundCommand::Info => {
                    ClientFrame::Info(pb::Info::decode_payload(&payload_bytes)?)
                }
                ClientInboundCommand::Message => {
                    ClientFrame::Message(pb::Message::decode_payload(&payload_bytes)?)
                }
            };
            return Ok(Some(frame));
        }
    }
}

impl<T> Encoder<T> for ClientCodec
where
    T: CommandCodec,
{
    type Error = ClientCodecError;

    fn encode(&mut self, item: T, output_buffer: &mut BytesMut) -> Result<(), Self::Error> {
        let payload = item.encode_payload()?;
        let payload_length: u32 =
            payload.len().try_into().map_err(|_| CodecError::InvalidSizeBytes(payload.len()))?;

        output_buffer.reserve(HEADER_LENGTH + payload.len());
        output_buffer.put_u8(T::COMMAND);
        output_buffer.put_u32(payload_length);
        output_buffer.extend_from_slice(&payload);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use prost::Message;
    use tokio_stream::StreamExt;
    use tokio_util::codec::FramedRead;

    use super::*;

    #[test]
    fn encode_info_frame_has_header_and_payload() {
        let info = pb::Info {
            version: 1,
            server_id: "srv-1".to_string(),
            server_name: "ocypode".to_string(),
            max_payload: 1024,
            client_id: 0,
            requires_auth: false,
            tls_verify: false,
        };
        let mut codec = ServerCodec;
        let mut output_buffer = BytesMut::new();

        codec.encode(info.clone(), &mut output_buffer).unwrap();

        assert!(output_buffer.len() >= HEADER_LENGTH);
        assert_eq!(output_buffer[0], Command::Info as u8);

        let mut header_bytes = &output_buffer[COMMAND_BYTE_LEN..HEADER_LENGTH];
        let payload_length = header_bytes.get_u32() as usize;
        let payload_bytes = &output_buffer[HEADER_LENGTH..];
        assert_eq!(payload_length, payload_bytes.len());

        let decoded = pb::Info::decode(payload_bytes).unwrap();
        assert_eq!(decoded.version, info.version);
        assert_eq!(decoded.server_id, info.server_id);
    }

    #[test]
    fn decode_conn_frame_recovers_from_bad_prefix() {
        let conn = pb::Connect {
            version: 1,
            verbose: true,
            auth_method: pb::AuthMethod::NoAuth as i32,
            credentials: None,
        };
        let payload = conn.encode_to_vec();

        let invalid_command_byte = 0xFF; // intentionally invalid to force resync
        let mut incoming_bytes = BytesMut::new();
        incoming_bytes.put_u8(invalid_command_byte);
        incoming_bytes.put_u8(Command::Connect as u8);
        incoming_bytes.put_u32(payload.len() as u32);
        incoming_bytes.extend_from_slice(&payload);

        let mut codec = ServerCodec;
        let decoded = codec.decode(&mut incoming_bytes).unwrap().unwrap();
        assert!(matches!(decoded, Frame::Connect(_)));
        assert!(incoming_bytes.is_empty());
    }

    #[test]
    fn encode_and_decode_info_frame() {
        let info = pb::Info {
            version: 1,
            server_id: "srv-2".to_string(),
            server_name: "ocypode".to_string(),
            max_payload: 2048,
            client_id: 0,
            requires_auth: false,
            tls_verify: false,
        };
        let mut server_codec = ServerCodec;
        let mut client_codec = ClientCodec;
        let mut output_buffer = BytesMut::new();

        server_codec.encode(info.clone(), &mut output_buffer).unwrap();

        let decoded = client_codec.decode(&mut output_buffer).unwrap().unwrap();
        match decoded {
            ClientFrame::Info(message) => {
                assert_eq!(message.server_id, info.server_id);
                assert_eq!(message.max_payload, info.max_payload);
            }
            ClientFrame::Message(_) => panic!("unexpected Message frame"),
        }
        assert!(output_buffer.is_empty());
    }

    #[test]
    fn client_encode_connect_frame_has_header_and_payload() {
        let conn = pb::Connect {
            version: 1,
            verbose: true,
            auth_method: pb::AuthMethod::NoAuth as i32,
            credentials: None,
        };
        let mut codec = ClientCodec;
        let mut output_buffer = BytesMut::new();

        codec.encode(conn.clone(), &mut output_buffer).unwrap();

        assert!(output_buffer.len() >= HEADER_LENGTH);
        assert_eq!(output_buffer[0], Command::Connect as u8);

        let mut header_bytes = &output_buffer[COMMAND_BYTE_LEN..HEADER_LENGTH];
        let payload_length = header_bytes.get_u32() as usize;
        let payload_bytes = &output_buffer[HEADER_LENGTH..];
        assert_eq!(payload_length, payload_bytes.len());

        let decoded = pb::Connect::decode(payload_bytes).unwrap();
        assert_eq!(decoded.version, conn.version);
    }

    #[test]
    fn client_decode_info_frame_recovers_from_bad_prefix() {
        let info = pb::Info {
            version: 1,
            server_id: "srv-3".to_string(),
            server_name: "ocypode".to_string(),
            max_payload: 512,
            client_id: 0,
            requires_auth: false,
            tls_verify: false,
        };
        let payload = info.encode_to_vec();

        let invalid_command_byte = 0xFF; // intentionally invalid to force resync
        let mut incoming_bytes = BytesMut::new();
        incoming_bytes.put_u8(invalid_command_byte);
        incoming_bytes.put_u8(Command::Info as u8);
        incoming_bytes.put_u32(payload.len() as u32);
        incoming_bytes.extend_from_slice(&payload);

        let mut codec = ClientCodec;
        let decoded = codec.decode(&mut incoming_bytes).unwrap().unwrap();
        match decoded {
            ClientFrame::Info(message) => {
                assert_eq!(message.server_id, info.server_id);
            }
            ClientFrame::Message(_) => panic!("unexpected Message frame"),
        }
        assert!(incoming_bytes.is_empty());
    }

    #[test]
    fn client_encode_and_decode_info_frame() {
        let info = pb::Info {
            version: 2,
            server_id: "srv-4".to_string(),
            server_name: "ocypode".to_string(),
            max_payload: 4096,
            client_id: 0,
            requires_auth: false,
            tls_verify: false,
        };
        let mut client_codec = ClientCodec;
        let mut server_codec = ServerCodec;
        let mut output_buffer = BytesMut::new();

        server_codec.encode(info.clone(), &mut output_buffer).unwrap();

        let decoded = client_codec.decode(&mut output_buffer).unwrap().unwrap();
        match decoded {
            ClientFrame::Info(message) => {
                assert_eq!(message.server_id, info.server_id);
                assert_eq!(message.max_payload, info.max_payload);
            }
            ClientFrame::Message(_) => panic!("unexpected Message frame"),
        }
        assert!(output_buffer.is_empty());
    }

    fn build_connect_frame() -> Vec<u8> {
        let conn = pb::Connect {
            version: 1,
            verbose: false,
            auth_method: pb::AuthMethod::NoAuth as i32,
            credentials: None,
        };
        let mut codec = ClientCodec;
        let mut buf = BytesMut::new();
        codec.encode(conn, &mut buf).unwrap();
        buf.to_vec()
    }

    #[tokio::test]
    async fn framed_read_decodes_single_connect_frame() {
        let data = build_connect_frame();
        let cursor = Cursor::new(data);
        let mut framed = FramedRead::with_capacity(cursor, ServerCodec, 32 * 1024);

        let frame = framed.next().await.unwrap().unwrap();
        assert!(matches!(frame, Frame::Connect(_)));
        assert!(framed.next().await.is_none());
    }

    #[tokio::test]
    async fn framed_read_decodes_multiple_frames_in_sequence() {
        let mut data = build_connect_frame();
        data.extend(build_connect_frame());
        let cursor = Cursor::new(data);
        let mut framed = FramedRead::with_capacity(cursor, ServerCodec, 32 * 1024);

        let frame1 = framed.next().await.unwrap().unwrap();
        assert!(matches!(frame1, Frame::Connect(_)));

        let frame2 = framed.next().await.unwrap().unwrap();
        assert!(matches!(frame2, Frame::Connect(_)));

        assert!(framed.next().await.is_none());
    }

    #[tokio::test]
    async fn framed_read_recovers_from_bad_prefix_byte() {
        let conn_data = build_connect_frame();
        let mut data = vec![0xFF]; // invalid command byte
        data.extend(conn_data);
        let cursor = Cursor::new(data);
        let mut framed = FramedRead::with_capacity(cursor, ServerCodec, 32 * 1024);

        let frame = framed.next().await.unwrap().unwrap();
        assert!(matches!(frame, Frame::Connect(_)));
        assert!(framed.next().await.is_none());
    }

    // --- Publish ---

    #[test]
    fn encode_and_decode_publish_frame() {
        let publish = pb::Publish {
            topic: b"sensors/temperature".to_vec(),
            payload: b"42.5".to_vec(),
            header: b"content-type:text/plain".to_vec(),
        };
        let mut server_codec = ServerCodec;
        let mut output_buffer = BytesMut::new();

        server_codec.encode(publish.clone(), &mut output_buffer).unwrap();

        let decoded = server_codec.decode(&mut output_buffer).unwrap().unwrap();
        let Frame::Publish(message) = decoded else { panic!("expected Publish frame") };
        assert_eq!(message.topic, publish.topic);
        assert_eq!(message.payload, publish.payload);
        assert_eq!(message.header, publish.header);
        assert!(output_buffer.is_empty());
    }

    #[test]
    fn encode_publish_frame_has_correct_header() {
        let publish = pb::Publish {
            topic: b"test/topic".to_vec(),
            payload: b"hello".to_vec(),
            header: vec![],
        };
        let mut codec = ServerCodec;
        let mut output_buffer = BytesMut::new();

        codec.encode(publish, &mut output_buffer).unwrap();

        assert!(output_buffer.len() >= HEADER_LENGTH);
        assert_eq!(output_buffer[0], Command::Publish as u8);

        let mut header_bytes = &output_buffer[COMMAND_BYTE_LEN..HEADER_LENGTH];
        let payload_length = header_bytes.get_u32() as usize;
        assert_eq!(payload_length, output_buffer.len() - HEADER_LENGTH);
    }

    // --- Subscribe ---

    #[test]
    fn encode_and_decode_subscribe_frame() {
        let subscribe = pb::Subscribe {
            topic: b"sensors/#".to_vec(),
            subscription_id: 7,
            queue_group: "workers".to_string(),
        };
        let mut server_codec = ServerCodec;
        let mut output_buffer = BytesMut::new();

        server_codec.encode(subscribe.clone(), &mut output_buffer).unwrap();

        let decoded = server_codec.decode(&mut output_buffer).unwrap().unwrap();
        let Frame::Subscribe(message) = decoded else { panic!("expected Subscribe frame") };
        assert_eq!(message.topic, subscribe.topic);
        assert_eq!(message.subscription_id, subscribe.subscription_id);
        assert_eq!(message.queue_group, subscribe.queue_group);
        assert!(output_buffer.is_empty());
    }

    #[test]
    fn subscribe_without_queue_group_roundtrips() {
        let subscribe = pb::Subscribe {
            topic: b"events/+/status".to_vec(),
            subscription_id: 1,
            queue_group: String::new(),
        };
        let mut server_codec = ServerCodec;
        let mut output_buffer = BytesMut::new();

        server_codec.encode(subscribe.clone(), &mut output_buffer).unwrap();
        let decoded = server_codec.decode(&mut output_buffer).unwrap().unwrap();
        let Frame::Subscribe(message) = decoded else { panic!("expected Subscribe frame") };
        assert_eq!(message.subscription_id, subscribe.subscription_id);
        assert!(message.queue_group.is_empty());
    }

    // --- UnSubscribe ---

    #[test]
    fn encode_and_decode_unsubscribe_frame() {
        let unsubscribe = pb::UnSubscribe { subscription_id: 42 };
        let mut server_codec = ServerCodec;
        let mut output_buffer = BytesMut::new();

        server_codec.encode(unsubscribe.clone(), &mut output_buffer).unwrap();

        let decoded = server_codec.decode(&mut output_buffer).unwrap().unwrap();
        let Frame::UnSubscribe(message) = decoded else { panic!("expected UnSubscribe frame") };
        assert_eq!(message.subscription_id, unsubscribe.subscription_id);
        assert!(output_buffer.is_empty());
    }

    // --- Message ---

    #[test]
    fn encode_and_decode_message_frame() {
        let message = pb::Message {
            topic: b"sensors/temperature".to_vec(),
            subscription_id: 3,
            payload: b"23.1".to_vec(),
            header: b"encoding:utf-8".to_vec(),
        };
        let mut server_codec = ServerCodec;
        let mut client_codec = ClientCodec;
        let mut output_buffer = BytesMut::new();

        server_codec.encode(message.clone(), &mut output_buffer).unwrap();

        let decoded = client_codec.decode(&mut output_buffer).unwrap().unwrap();
        let ClientFrame::Message(delivered) = decoded else { panic!("expected Message frame") };
        assert_eq!(delivered.topic, message.topic);
        assert_eq!(delivered.subscription_id, message.subscription_id);
        assert_eq!(delivered.payload, message.payload);
        assert_eq!(delivered.header, message.header);
        assert!(output_buffer.is_empty());
    }

    #[test]
    fn client_decode_message_frame_recovers_from_bad_prefix() {
        let message = pb::Message {
            topic: b"test/topic".to_vec(),
            subscription_id: 5,
            payload: b"data".to_vec(),
            header: vec![],
        };
        let payload = message.encode_to_vec();

        let mut incoming_bytes = BytesMut::new();
        incoming_bytes.put_u8(0xFF); // invalid command byte to force resync
        incoming_bytes.put_u8(Command::Message as u8);
        incoming_bytes.put_u32(payload.len() as u32);
        incoming_bytes.extend_from_slice(&payload);

        let mut codec = ClientCodec;
        let decoded = codec.decode(&mut incoming_bytes).unwrap().unwrap();
        let ClientFrame::Message(delivered) = decoded else { panic!("expected Message frame") };
        assert_eq!(delivered.subscription_id, message.subscription_id);
        assert!(incoming_bytes.is_empty());
    }

    // --- Mixed frame sequence ---

    #[tokio::test]
    async fn framed_read_decodes_publish_subscribe_unsubscribe_sequence() {
        let publish = pb::Publish {
            topic: b"a/b".to_vec(),
            payload: b"payload".to_vec(),
            header: vec![],
        };
        let subscribe = pb::Subscribe {
            topic: b"a/#".to_vec(),
            subscription_id: 1,
            queue_group: String::new(),
        };
        let unsubscribe = pb::UnSubscribe { subscription_id: 1 };

        let mut client_codec = ClientCodec;
        let mut buf = BytesMut::new();
        client_codec.encode(publish, &mut buf).unwrap();
        client_codec.encode(subscribe, &mut buf).unwrap();
        client_codec.encode(unsubscribe, &mut buf).unwrap();

        let cursor = Cursor::new(buf.to_vec());
        let mut framed = FramedRead::with_capacity(cursor, ServerCodec, 32 * 1024);

        assert!(matches!(framed.next().await.unwrap().unwrap(), Frame::Publish(_)));
        assert!(matches!(framed.next().await.unwrap().unwrap(), Frame::Subscribe(_)));
        assert!(matches!(framed.next().await.unwrap().unwrap(), Frame::UnSubscribe(_)));
        assert!(framed.next().await.is_none());
    }
}
