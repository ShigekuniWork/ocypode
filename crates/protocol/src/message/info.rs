use bytes::{BufMut, Bytes, BytesMut};

use crate::{Command, codec::CommandCodec, error::DecodeError, wire::WireDecode};

const AUTH_REQUIRED_BIT: u8 = 0x01;
const HEADERS_BIT: u8 = 0x02;

/// Sent by the server immediately after a QUIC connection is established.
pub struct Info {
    pub version: u8,
    pub max_payload: u32,
    pub server_id: Bytes,
    pub server_name: Bytes,
    pub auth_required: bool,
    pub headers: bool,
}

impl CommandCodec for Info {
    fn command() -> Command {
        Command::INFO
    }

    fn encode(&self, dst: &mut BytesMut) {
        dst.put_u8(self.version);
        dst.put_u32(self.max_payload);

        dst.put_u8(self.server_id.len() as u8);
        dst.put(self.server_id.as_ref());

        dst.put_u8(self.server_name.len() as u8);
        dst.put(self.server_name.as_ref());

        let capability_flags =
            ((self.auth_required as u8) * AUTH_REQUIRED_BIT) | ((self.headers as u8) * HEADERS_BIT);
        dst.put_u8(capability_flags);
    }

    fn decode(_flags: u8, src: &mut Bytes) -> Result<Self, DecodeError> {
        let version = src.read_u8()?;
        let max_payload = src.read_u32()?;
        let server_id = src.read_length_prefixed_u8()?;
        let server_name = src.read_length_prefixed_u8()?;
        let capability_flags = src.read_u8()?;

        Ok(Self {
            version,
            max_payload,
            server_id,
            server_name,
            auth_required: capability_flags & AUTH_REQUIRED_BIT != 0,
            headers: capability_flags & HEADERS_BIT != 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use bytes::BytesMut;

    use super::*;

    fn make_info() -> Info {
        Info {
            version: 1,
            max_payload: 1_048_576,
            server_id: Bytes::from_static(b"server-abc"),
            server_name: Bytes::from_static(b"my-server"),
            auth_required: true,
            headers: false,
        }
    }

    #[test]
    fn encode_decode_roundtrip() {
        let original = make_info();
        let mut buf = BytesMut::new();
        original.encode(&mut buf);
        let mut bytes = buf.freeze();
        let decoded = Info::decode(0, &mut bytes).unwrap();

        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.max_payload, 1_048_576);
        assert_eq!(decoded.server_id, Bytes::from_static(b"server-abc"));
        assert_eq!(decoded.server_name, Bytes::from_static(b"my-server"));
        assert!(decoded.auth_required);
        assert!(!decoded.headers);
    }

    #[test]
    fn capability_flags_auth_required_and_headers() {
        let info = Info {
            version: 1,
            max_payload: 0,
            server_id: Bytes::new(),
            server_name: Bytes::new(),
            auth_required: true,
            headers: true,
        };
        let mut buf = BytesMut::new();
        info.encode(&mut buf);
        let mut bytes = buf.freeze();
        let decoded = Info::decode(0, &mut bytes).unwrap();

        assert!(decoded.auth_required);
        assert!(decoded.headers);
    }

    #[test]
    fn capability_flags_none_set() {
        let info = Info {
            version: 1,
            max_payload: 0,
            server_id: Bytes::new(),
            server_name: Bytes::new(),
            auth_required: false,
            headers: false,
        };
        let mut buf = BytesMut::new();
        info.encode(&mut buf);
        let mut bytes = buf.freeze();
        let decoded = Info::decode(0, &mut bytes).unwrap();

        assert!(!decoded.auth_required);
        assert!(!decoded.headers);
    }
}
