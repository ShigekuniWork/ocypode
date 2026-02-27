use bytes::{Bytes, BytesMut};

use crate::{
    Command,
    error::DecodeError,
    header::FixedHeader,
    message::{Message, info::Info},
};

const FLAGS_NONE: u8 = 0;
const FIXED_HEADER_MAX_LEN: usize = 5;

/// Per-command codec. Each command type implements this trait to keep
/// its own parsing and serialization logic self-contained.
pub trait CommandCodec: Sized {
    fn command() -> Command;
    fn encode(&self, dst: &mut BytesMut);
    fn decode(flags: u8, src: &mut Bytes) -> Result<Self, DecodeError>;
}

/// Sends protocol messages to clients over an established connection.
pub struct ServerCodec;

impl ServerCodec {
    /// Serializes a server→client [`Message`] into a wire frame ready for
    /// transmission.
    pub fn encode(&self, message: &Message) -> Bytes {
        let mut payload = BytesMut::new();

        match message {
            Message::Info(info) => {
                info.encode(&mut payload);
                let header = FixedHeader::new(Command::INFO, FLAGS_NONE, payload.len() as u32);
                let mut wire = BytesMut::with_capacity(FIXED_HEADER_MAX_LEN + payload.len());
                header.encode(&mut wire);
                wire.unsplit(payload);
                wire.freeze()
            }
        }
    }
}

/// Receives protocol messages from the server over an established connection.
pub struct ClientCodec;

impl ClientCodec {
    /// Parses a wire frame received from the server into a server→client
    /// [`Message`].
    pub fn decode(&self, mut src: Bytes) -> Result<Message, DecodeError> {
        let header = FixedHeader::decode(&mut src)?;

        match header.command {
            Command::INFO => {
                let info = Info::decode(header.flags, &mut src)?;
                Ok(Message::Info(info))
            }
            other => Err(DecodeError::UnsupportedCommand(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_encode_client_decode_info_roundtrip() {
        let original = Info {
            version: 1,
            max_payload: 1_048_576,
            server_id: Bytes::from_static(b"srv-1"),
            server_name: Bytes::from_static(b"ocypode"),
            auth_required: true,
            headers: false,
        };

        let wire = ServerCodec.encode(&Message::Info(original));
        let message = ClientCodec.decode(wire).unwrap();

        let Message::Info(decoded) = message;
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.max_payload, 1_048_576);
        assert_eq!(decoded.server_id, Bytes::from_static(b"srv-1"));
        assert_eq!(decoded.server_name, Bytes::from_static(b"ocypode"));
        assert!(decoded.auth_required);
        assert!(!decoded.headers);
    }
}
