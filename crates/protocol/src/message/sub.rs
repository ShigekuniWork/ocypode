use bytes::{Bytes, BytesMut};
use topic::TopicFilter;

use crate::{
    Command,
    error::DecodeError,
    wire::{CommandCodec, WireDecode, WireEncode},
};

const HAS_QUEUE_GROUP_BIT: u8 = 0x01;

/// Sent by the client to subscribe to a topic or wildcard topic.
pub struct Sub {
    pub topic: TopicFilter,
    pub subscription_id: Bytes,
    pub queue_group: Option<Bytes>,
}

impl CommandCodec for Sub {
    fn command() -> Command {
        Command::SUB
    }

    fn flags(&self) -> u8 {
        if self.queue_group.is_some() { HAS_QUEUE_GROUP_BIT } else { 0 }
    }

    fn encode(&self, dst: &mut BytesMut) {
        self.topic.encode_to(dst);
        dst.put_length_prefixed_u16(self.subscription_id.as_ref());
        if let Some(queue_group) = &self.queue_group {
            dst.put_length_prefixed_u8(queue_group.as_ref());
        }
    }

    fn decode(flags: u8, src: &mut Bytes) -> Result<Self, DecodeError> {
        let has_queue_group = flags & HAS_QUEUE_GROUP_BIT != 0;

        let topic = TopicFilter::decode(src)?;
        let subscription_id = src.read_length_prefixed_u16()?;

        // TODO: think specification
        let queue_group = if has_queue_group { Some(src.read_length_prefixed_u8()?) } else { None };

        Ok(Self { topic, subscription_id, queue_group })
    }
}

#[cfg(test)]
mod tests {
    use bytes::{Bytes, BytesMut};
    use wire::WireEncode;

    use super::*;

    fn make_topic_filter(raw: &[u8]) -> TopicFilter {
        let mut buf = BytesMut::new();
        buf.put_length_prefixed_u16(raw);
        TopicFilter::decode(&mut buf.freeze()).unwrap()
    }

    fn encode_sub(message: &Sub) -> Bytes {
        let mut buf = BytesMut::new();
        message.encode(&mut buf);
        buf.freeze()
    }

    #[test]
    fn encode_decode_without_queue_group_roundtrip() {
        let original = Sub {
            topic: make_topic_filter(b"events/orders"),
            subscription_id: Bytes::from_static(b"sub-1"),
            queue_group: None,
        };
        let mut bytes = encode_sub(&original);
        let decoded = Sub::decode(original.flags(), &mut bytes).unwrap();

        assert_eq!(decoded.topic.as_bytes(), &Bytes::from_static(b"events/orders"));
        assert_eq!(decoded.subscription_id, Bytes::from_static(b"sub-1"));
        assert!(decoded.queue_group.is_none());
    }

    #[test]
    fn has_queue_group_flag_set() {
        let message = Sub {
            topic: make_topic_filter(b"topic"),
            subscription_id: Bytes::from_static(b"id"),
            queue_group: Some(Bytes::from_static(b"workers")),
        };
        assert_eq!(message.flags() & HAS_QUEUE_GROUP_BIT, HAS_QUEUE_GROUP_BIT);
    }

    #[test]
    fn has_queue_group_flag_unset() {
        let message = Sub {
            topic: make_topic_filter(b"topic"),
            subscription_id: Bytes::from_static(b"id"),
            queue_group: None,
        };
        assert_eq!(message.flags() & HAS_QUEUE_GROUP_BIT, 0);
    }

    #[test]
    fn encode_decode_with_queue_group_roundtrip() {
        let original = Sub {
            topic: make_topic_filter(b"tasks/+"),
            subscription_id: Bytes::from_static(b"sub-42"),
            queue_group: Some(Bytes::from_static(b"workers")),
        };
        let mut bytes = encode_sub(&original);
        let decoded = Sub::decode(original.flags(), &mut bytes).unwrap();

        assert_eq!(decoded.topic.as_bytes(), &Bytes::from_static(b"tasks/+"));
        assert_eq!(decoded.subscription_id, Bytes::from_static(b"sub-42"));
        assert_eq!(decoded.queue_group.unwrap(), Bytes::from_static(b"workers"));
    }

    #[test]
    fn encode_decode_wildcard_filter_roundtrip() {
        let original = Sub {
            topic: make_topic_filter(b"sensor/+/#"),
            subscription_id: Bytes::from_static(b"sub-99"),
            queue_group: None,
        };
        let mut bytes = encode_sub(&original);
        let decoded = Sub::decode(original.flags(), &mut bytes).unwrap();

        assert_eq!(decoded.topic.as_bytes(), &Bytes::from_static(b"sensor/+/#"));
    }
}
