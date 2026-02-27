use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::{Command, error::DecodeError};

const COMMAND_SHIFT: u8 = 4;
const FLAGS_MASK: u8 = 0x0F;
const VARINT_DATA_MASK: u8 = 0x7F;
const VARINT_CONTINUATION_BIT: u8 = 0x80;
const VARINT_DATA_BITS: u32 = 7;
const VARINT_MAX_BYTES: usize = 4;

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
        let remaining_length = decode_variable_length(src)?;

        Ok(Self { command, flags, remaining_length })
    }

    pub fn encode(&self, dst: &mut BytesMut) {
        dst.put_u8((self.command as u8) << COMMAND_SHIFT | (self.flags & FLAGS_MASK));
        encode_variable_length(self.remaining_length, dst);
    }
}

/// Decodes a variable-length integer where the MSB of each byte is a
/// continuation flag and the lower 7 bits encode the value. Supports 1–4 bytes.
fn decode_variable_length(src: &mut Bytes) -> Result<u32, DecodeError> {
    let mut value: u32 = 0;
    let mut shift = 0u32;

    for _ in 0..VARINT_MAX_BYTES {
        if !src.has_remaining() {
            return Err(DecodeError::BufferTooShort { expected: 1, actual: 0 });
        }
        let byte = src.get_u8();
        value |= ((byte & VARINT_DATA_MASK) as u32) << shift;
        if byte & VARINT_CONTINUATION_BIT == 0 {
            return Ok(value);
        }
        shift += VARINT_DATA_BITS;
    }

    Err(DecodeError::VariableLengthOverflow)
}

/// Encodes a u32 as a variable-length integer into `dst`.
fn encode_variable_length(mut value: u32, dst: &mut BytesMut) {
    loop {
        let mut byte = (value & VARINT_DATA_MASK as u32) as u8;
        value >>= VARINT_DATA_BITS;
        if value > 0 {
            byte |= VARINT_CONTINUATION_BIT;
        }
        dst.put_u8(byte);
        if value == 0 {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_length_single_byte_roundtrip() {
        let mut buf = BytesMut::new();
        encode_variable_length(127, &mut buf);
        let mut bytes = buf.freeze();
        assert_eq!(decode_variable_length(&mut bytes).unwrap(), 127);
    }

    #[test]
    fn variable_length_multi_byte_roundtrip() {
        let mut buf = BytesMut::new();
        encode_variable_length(16_383, &mut buf);
        let mut bytes = buf.freeze();
        assert_eq!(decode_variable_length(&mut bytes).unwrap(), 16_383);
    }

    #[test]
    fn variable_length_max_roundtrip() {
        let mut buf = BytesMut::new();
        encode_variable_length(268_435_455, &mut buf);
        let mut bytes = buf.freeze();
        assert_eq!(decode_variable_length(&mut bytes).unwrap(), 268_435_455);
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
