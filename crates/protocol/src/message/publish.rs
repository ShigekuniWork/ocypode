use bytes::{Buf, BufMut, Bytes, BytesMut};
use topic::Topic;

use crate::{
    Command,
    error::DecodeError,
    wire::{CommandCodec, Headers, WireDecode, WireEncode},
};

const HAS_REPLY_TO_BIT: u8 = 0x01;
const HAS_HEADER_BIT: u8 = 0x02;

/// Sent by the client to publish a message to a specified topic.
pub struct Pub {
    pub topic: Topic,
    pub reply_to: Option<Topic>,
    pub header: Option<Headers>,
    pub payload: Bytes,
}

impl CommandCodec for Pub {
    fn command() -> Command {
        Command::PUB
    }

    fn flags(&self) -> u8 {
        let mut flags = 0u8;
        if self.reply_to.is_some() {
            flags |= HAS_REPLY_TO_BIT;
        }
        if self.header.is_some() {
            flags |= HAS_HEADER_BIT;
        }
        flags
    }

    fn encode(&self, dst: &mut BytesMut) {
        self.topic.encode_to(dst);
        if let Some(reply_to) = &self.reply_to {
            reply_to.encode_to(dst);
        }
        if let Some(header) = &self.header {
            let mut header_bytes = BytesMut::new();
            header.encode_to(&mut header_bytes);
            dst.put_length_prefixed_u16(&header_bytes);
        }
        dst.put_varint(self.payload.len() as u32);
        dst.put(self.payload.as_ref());
    }

    fn decode(flags: u8, src: &mut Bytes) -> Result<Self, DecodeError> {
        let has_reply_to = flags & HAS_REPLY_TO_BIT != 0;
        let has_header = flags & HAS_HEADER_BIT != 0;

        let topic = Topic::decode(src)?;

        let reply_to = if has_reply_to { Some(Topic::decode(src)?) } else { None };

        let header = if has_header {
            let mut header_bytes = src.read_length_prefixed_u16()?;
            Some(Headers::decode_from(&mut header_bytes)?)
        } else {
            None
        };

        let payload_size = src.read_varint()? as usize;
        if src.remaining() < payload_size {
            return Err(DecodeError::BufferTooShort {
                expected: payload_size,
                actual: src.remaining(),
            });
        }
        let payload = src.copy_to_bytes(payload_size);

        Ok(Self { topic, reply_to, header, payload })
    }
}

#[cfg(test)]
mod tests {
    use bytes::{Bytes, BytesMut};

    use super::*;
    use crate::wire::{Headers, WireEncode};

    fn make_topic(raw: &[u8]) -> Topic {
        let mut buf = BytesMut::new();
        buf.put_length_prefixed_u16(raw);
        Topic::decode(&mut buf.freeze()).unwrap()
    }

    fn encode_pub(message: &Pub) -> Bytes {
        let mut buf = BytesMut::new();
        message.encode(&mut buf);
        buf.freeze()
    }

    fn make_header(key: &'static [u8], value: &'static [u8]) -> Headers {
        let mut h = Headers::new();
        h.insert(Bytes::from_static(key), Bytes::from_static(value));
        h
    }

    #[test]
    fn encode_decode_minimal_roundtrip() {
        let original = Pub {
            topic: make_topic(b"events/orders"),
            reply_to: None,
            header: None,
            payload: Bytes::from_static(b"hello"),
        };
        let mut bytes = encode_pub(&original);
        let decoded = Pub::decode(original.flags(), &mut bytes).unwrap();

        assert_eq!(decoded.topic.as_bytes(), &Bytes::from_static(b"events/orders"));
        assert!(decoded.reply_to.is_none());
        assert!(decoded.header.is_none());
        assert_eq!(decoded.payload, Bytes::from_static(b"hello"));
    }

    #[test]
    fn has_reply_to_flag_set() {
        let message = Pub {
            topic: make_topic(b"topic"),
            reply_to: Some(make_topic(b"reply")),
            header: None,
            payload: Bytes::new(),
        };
        assert_eq!(message.flags() & HAS_REPLY_TO_BIT, HAS_REPLY_TO_BIT);
    }

    #[test]
    fn has_reply_to_flag_unset() {
        let message = Pub {
            topic: make_topic(b"topic"),
            reply_to: None,
            header: None,
            payload: Bytes::new(),
        };
        assert_eq!(message.flags() & HAS_REPLY_TO_BIT, 0);
    }

    #[test]
    fn has_header_flag_set() {
        let message = Pub {
            topic: make_topic(b"topic"),
            reply_to: None,
            header: Some(make_header(b"key", b"value")),
            payload: Bytes::new(),
        };
        assert_eq!(message.flags() & HAS_HEADER_BIT, HAS_HEADER_BIT);
    }

    #[test]
    fn has_header_flag_unset() {
        let message = Pub {
            topic: make_topic(b"topic"),
            reply_to: None,
            header: None,
            payload: Bytes::new(),
        };
        assert_eq!(message.flags() & HAS_HEADER_BIT, 0);
    }

    #[test]
    fn encode_decode_with_reply_to_roundtrip() {
        let original = Pub {
            topic: make_topic(b"requests"),
            reply_to: Some(make_topic(b"replies/123")),
            header: None,
            payload: Bytes::from_static(b"ping"),
        };
        let mut bytes = encode_pub(&original);
        let decoded = Pub::decode(original.flags(), &mut bytes).unwrap();

        assert_eq!(decoded.topic.as_bytes(), &Bytes::from_static(b"requests"));
        assert_eq!(decoded.reply_to.unwrap().as_bytes(), &Bytes::from_static(b"replies/123"));
        assert!(decoded.header.is_none());
        assert_eq!(decoded.payload, Bytes::from_static(b"ping"));
    }

    #[test]
    fn encode_decode_with_single_header_roundtrip() {
        let mut header = Headers::new();
        header.insert(Bytes::from_static(b"content-type"), Bytes::from_static(b"application/json"));
        let original = Pub {
            topic: make_topic(b"metrics"),
            reply_to: None,
            header: Some(header),
            payload: Bytes::from_static(b"{\"value\":42}"),
        };
        let mut bytes = encode_pub(&original);
        let decoded = Pub::decode(original.flags(), &mut bytes).unwrap();

        assert_eq!(decoded.topic.as_bytes(), &Bytes::from_static(b"metrics"));
        assert!(decoded.reply_to.is_none());
        assert_eq!(decoded.payload, Bytes::from_static(b"{\"value\":42}"));
        let entries = decoded.header.unwrap();
        let entries = entries.entries();
        assert_eq!(entries[0].0, Bytes::from_static(b"content-type"));
        assert_eq!(entries[0].1, Bytes::from_static(b"application/json"));
    }

    #[test]
    fn encode_decode_with_multiple_headers_preserves_order() {
        let mut header = Headers::new();
        header.insert(Bytes::from_static(b"trace-id"), Bytes::from_static(b"xyz"));
        header.insert(Bytes::from_static(b"content-type"), Bytes::from_static(b"application/json"));
        let original = Pub {
            topic: make_topic(b"rpc/add"),
            reply_to: Some(make_topic(b"inbox/abc")),
            header: Some(header),
            payload: Bytes::from_static(b"1+2"),
        };
        let mut bytes = encode_pub(&original);
        let decoded = Pub::decode(original.flags(), &mut bytes).unwrap();

        assert_eq!(decoded.topic.as_bytes(), &Bytes::from_static(b"rpc/add"));
        assert_eq!(decoded.reply_to.unwrap().as_bytes(), &Bytes::from_static(b"inbox/abc"));
        assert_eq!(decoded.payload, Bytes::from_static(b"1+2"));
        let entries = decoded.header.unwrap();
        let entries = entries.entries();
        assert_eq!(entries[0], (Bytes::from_static(b"trace-id"), Bytes::from_static(b"xyz")));
        assert_eq!(
            entries[1],
            (Bytes::from_static(b"content-type"), Bytes::from_static(b"application/json"))
        );
    }
}
