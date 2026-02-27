use bytes::{Bytes, BytesMut};

use crate::{
    Command,
    error::DecodeError,
    wire::{CommandCodec, WireDecode, WireEncode},
};

/// Sent by the client to end a subscription.
pub struct Unsub {
    pub subscription_id: Bytes,
}

impl CommandCodec for Unsub {
    fn command() -> Command {
        Command::UNSUB
    }

    fn encode(&self, dst: &mut BytesMut) {
        dst.put_length_prefixed_u16(self.subscription_id.as_ref());
    }

    fn decode(_flags: u8, src: &mut Bytes) -> Result<Self, DecodeError> {
        let subscription_id = src.read_length_prefixed_u16()?;
        Ok(Self { subscription_id })
    }
}

#[cfg(test)]
mod tests {
    use bytes::BytesMut;

    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        let original = Unsub { subscription_id: Bytes::from_static(b"sub-99") };
        let mut buf = BytesMut::new();
        original.encode(&mut buf);
        let mut bytes = buf.freeze();
        let decoded = Unsub::decode(0, &mut bytes).unwrap();

        assert_eq!(decoded.subscription_id, Bytes::from_static(b"sub-99"));
    }
}
