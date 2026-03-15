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

/// Command classify Ocypode protocol.
#[repr(u8)]
pub enum Command {
    Info = 0x00,
    Connect = 0x01,
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

#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    Connect(pb::Connect),
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum ClientFrame {
    Info(pb::Info),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerInboundCommand {
    Connect,
}

impl TryFrom<u8> for ServerInboundCommand {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value == <pb::Connect as CommandCodec>::COMMAND {
            Ok(ServerInboundCommand::Connect)
        } else {
            Err(())
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientInboundCommand {
    Info,
}

impl TryFrom<u8> for ClientInboundCommand {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value == <pb::Info as CommandCodec>::COMMAND {
            Ok(ClientInboundCommand::Info)
        } else {
            Err(())
        }
    }
}

/// Server outbound message builder
pub struct ServerOutbound;

impl ServerOutbound {
    /// Creates an INFO message with specified parameters
    pub fn info(version: u32, client_id: u64, server_id: String, server_name: String) -> pb::Info {
        pb::Info {
            version,
            auth_type: pb::info::AuthType::NoAuth as i32,
            server_id,
            server_name,
            max_payload: MAXIMUM_PAYLOAD_BYTES as u32,
            client_id,
        }
    }

    /// Creates a default INFO message
    /// TODO: Load INFO message from configuration instead of using dummy values
    #[allow(dead_code)]
    pub fn default_info() -> pb::Info {
        Self::info(1, 0, "ocypode-server".to_string(), "ocypode".to_string())
    }
}

/// Client outbound message builder
#[allow(dead_code)]
pub struct ClientOutbound;

impl ClientOutbound {
    /// Creates a CONNECT message with specified parameters
    #[allow(dead_code)]
    pub fn connect(version: u32, verbose: bool) -> pb::Connect {
        pb::Connect { version, verbose, credentials: None }
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
            auth_type: pb::info::AuthType::NoAuth as i32,
            server_id: "srv-1".to_string(),
            server_name: "ocypode".to_string(),
            max_payload: 1024,
            client_id: 0,
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
        let conn = pb::Connect { version: 1, verbose: true, credentials: None };
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
            auth_type: pb::info::AuthType::NoAuth as i32,
            server_id: "srv-2".to_string(),
            server_name: "ocypode".to_string(),
            max_payload: 2048,
            client_id: 0,
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
        }
        assert!(output_buffer.is_empty());
    }

    #[test]
    fn client_encode_connect_frame_has_header_and_payload() {
        let conn = pb::Connect { version: 1, verbose: true, credentials: None };
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
            auth_type: pb::info::AuthType::NoAuth as i32,
            server_id: "srv-3".to_string(),
            server_name: "ocypode".to_string(),
            max_payload: 512,
            client_id: 0,
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
        }
        assert!(incoming_bytes.is_empty());
    }

    #[test]
    fn client_encode_and_decode_info_frame() {
        let info = pb::Info {
            version: 2,
            auth_type: pb::info::AuthType::NoAuth as i32,
            server_id: "srv-4".to_string(),
            server_name: "ocypode".to_string(),
            max_payload: 4096,
            client_id: 0,
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
        }
        assert!(output_buffer.is_empty());
    }

    fn build_connect_frame() -> Vec<u8> {
        let conn = pb::Connect { version: 1, verbose: false, credentials: None };
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
}
