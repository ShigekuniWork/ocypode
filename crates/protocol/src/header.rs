use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::{
    Command,
    error::DecodeError,
    wire::{COMMAND_SHIFT, FLAGS_MASK, WireDecode, WireEncode},
};

/// Fixed header present in every protocol message.
///
/// Wire layout:
/// - Byte 0: upper 4 bits = command, lower 4 bits = flags
/// - Bytes 1–4: remaining_length (variable-length integer)
pub struct FixedHeader {
    pub command: Command,
    pub flags: u8,
    pub remaining_length: u32,
}

impl FixedHeader {
    pub fn new(command: Command, flags: u8, remaining_length: u32) -> Self {
        Self { command, flags, remaining_length }
    }

    pub fn decode(src: &mut Bytes) -> Result<Self, DecodeError> {
        if src.remaining() < 2 {
            return Err(DecodeError::BufferTooShort { expected: 2, actual: src.remaining() });
        }

        let first_byte = src.get_u8();
        let command = Command::try_from(first_byte >> COMMAND_SHIFT)?;
        let flags = first_byte & FLAGS_MASK;
        let remaining_length = src.read_varint()?;

        Ok(Self { command, flags, remaining_length })
    }

    pub fn encode(&self, dst: &mut BytesMut) {
        dst.put_u8((self.command as u8) << COMMAND_SHIFT | (self.flags & FLAGS_MASK));
        dst.put_varint(self.remaining_length);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{WireDecode, WireEncode};

    #[test]
    fn variable_length_single_byte_roundtrip() {
        let mut buf = BytesMut::new();
        buf.put_varint(127);
        let mut bytes = buf.freeze();
        assert_eq!(bytes.read_varint().unwrap(), 127);
    }

    #[test]
    fn variable_length_multi_byte_roundtrip() {
        let mut buf = BytesMut::new();
        buf.put_varint(16_383);
        let mut bytes = buf.freeze();
        assert_eq!(bytes.read_varint().unwrap(), 16_383);
    }

    #[test]
    fn variable_length_max_roundtrip() {
        let mut buf = BytesMut::new();
        buf.put_varint(268_435_455);
        let mut bytes = buf.freeze();
        assert_eq!(bytes.read_varint().unwrap(), 268_435_455);
    }

    #[test]
    fn fixed_header_roundtrip() {
        let header = FixedHeader::new(Command::INFO, 0, 42);
        let mut buf = BytesMut::new();
        header.encode(&mut buf);
        let mut bytes = buf.freeze();
        let decoded = FixedHeader::decode(&mut bytes).unwrap();
        assert_eq!(decoded.command, Command::INFO);
        assert_eq!(decoded.flags, 0);
        assert_eq!(decoded.remaining_length, 42);
    }
}
