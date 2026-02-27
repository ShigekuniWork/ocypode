use std::mem::size_of;

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::{Command, error::DecodeError};

pub(crate) const COMMAND_SHIFT: u8 = 4;
pub(crate) const FLAGS_MASK: u8 = 0x0F;
pub(crate) const FIXED_HEADER_MAX_LEN: usize = 5;

pub(crate) const VARINT_DATA_MASK: u8 = 0x7F;
pub(crate) const VARINT_CONTINUATION_BIT: u8 = 0x80;
pub(crate) const VARINT_DATA_BITS: u32 = 7;
pub(crate) const VARINT_MAX_BYTES: usize = 4;

/// A topic name, shared across PUB, SUB, MSG, and UNSUB commands.
/// Maximum 256 bytes.
/// TODO: Change Actual Topic
pub struct Topic(pub Bytes);

/// An opaque message payload, shared across PUB and MSG commands.
pub struct Payload(pub Bytes);

/// Key-value string headers in KV string format, shared across PUB and MSG commands.
pub struct Headers(pub Bytes);

/// Extension trait on [`Bytes`] for reading typed wire-protocol fields.
///
/// Provides method-chain style reads shared across all [`crate::codec::CommandCodec`]
/// implementations.
pub trait WireDecode {
    fn read_u8(&mut self) -> Result<u8, DecodeError>;
    fn read_u16(&mut self) -> Result<u16, DecodeError>;
    fn read_u32(&mut self) -> Result<u32, DecodeError>;
    /// Reads a variable-length integer (1–4 bytes, MSB = continuation flag).
    fn read_varint(&mut self) -> Result<u32, DecodeError>;
    /// Reads a 1-byte length prefix followed by that many bytes.
    fn read_length_prefixed_u8(&mut self) -> Result<Bytes, DecodeError>;
    /// Reads a 2-byte length prefix followed by that many bytes.
    fn read_length_prefixed_u16(&mut self) -> Result<Bytes, DecodeError>;
}

impl WireDecode for Bytes {
    fn read_u8(&mut self) -> Result<u8, DecodeError> {
        if self.remaining() < size_of::<u8>() {
            return Err(DecodeError::BufferTooShort {
                expected: size_of::<u8>(),
                actual: self.remaining(),
            });
        }
        Ok(self.get_u8())
    }

    fn read_u16(&mut self) -> Result<u16, DecodeError> {
        if self.remaining() < size_of::<u16>() {
            return Err(DecodeError::BufferTooShort {
                expected: size_of::<u16>(),
                actual: self.remaining(),
            });
        }
        Ok(self.get_u16())
    }

    fn read_u32(&mut self) -> Result<u32, DecodeError> {
        if self.remaining() < size_of::<u32>() {
            return Err(DecodeError::BufferTooShort {
                expected: size_of::<u32>(),
                actual: self.remaining(),
            });
        }
        Ok(self.get_u32())
    }

    fn read_varint(&mut self) -> Result<u32, DecodeError> {
        let mut value: u32 = 0;
        let mut shift = 0u32;
        for _ in 0..VARINT_MAX_BYTES {
            if !self.has_remaining() {
                return Err(DecodeError::BufferTooShort { expected: 1, actual: 0 });
            }
            let byte = self.get_u8();
            value |= ((byte & VARINT_DATA_MASK) as u32) << shift;
            if byte & VARINT_CONTINUATION_BIT == 0 {
                return Ok(value);
            }
            shift += VARINT_DATA_BITS;
        }
        Err(DecodeError::VariableLengthOverflow)
    }

    fn read_length_prefixed_u8(&mut self) -> Result<Bytes, DecodeError> {
        let length = self.read_u8()? as usize;
        if self.remaining() < length {
            return Err(DecodeError::BufferTooShort { expected: length, actual: self.remaining() });
        }
        Ok(self.copy_to_bytes(length))
    }

    fn read_length_prefixed_u16(&mut self) -> Result<Bytes, DecodeError> {
        let length = self.read_u16()? as usize;
        if self.remaining() < length {
            return Err(DecodeError::BufferTooShort { expected: length, actual: self.remaining() });
        }
        Ok(self.copy_to_bytes(length))
    }
}

/// Extension trait on [`BytesMut`] for writing typed wire-protocol fields.
///
/// Mirrors [`WireDecode`] so encode and decode stay symmetric across commands.
pub trait WireEncode {
    /// Writes a variable-length integer (1–4 bytes, MSB = continuation flag).
    fn put_varint(&mut self, value: u32);
    /// Writes a 1-byte length prefix followed by the given bytes.
    fn put_length_prefixed_u8(&mut self, bytes: impl AsRef<[u8]>);
    /// Writes a 2-byte length prefix followed by the given bytes.
    fn put_length_prefixed_u16(&mut self, bytes: impl AsRef<[u8]>);
}

impl WireEncode for BytesMut {
    fn put_varint(&mut self, mut value: u32) {
        loop {
            let mut byte = (value & VARINT_DATA_MASK as u32) as u8;
            value >>= VARINT_DATA_BITS;
            if value > 0 {
                byte |= VARINT_CONTINUATION_BIT;
            }
            self.put_u8(byte);
            if value == 0 {
                break;
            }
        }
    }

    fn put_length_prefixed_u8(&mut self, bytes: impl AsRef<[u8]>) {
        let bytes = bytes.as_ref();
        self.put_u8(bytes.len() as u8);
        self.put(bytes);
    }

    fn put_length_prefixed_u16(&mut self, bytes: impl AsRef<[u8]>) {
        let bytes = bytes.as_ref();
        self.put_u16(bytes.len() as u16);
        self.put(bytes);
    }
}

/// Per-command codec. Each command type implements this trait to keep
/// its own parsing and serialization logic self-contained.
pub trait CommandCodec: Sized {
    fn command() -> Command;
    /// Returns the flags byte for the fixed header. Defaults to `0`.
    fn flags(&self) -> u8 {
        0
    }
    fn encode(&self, dst: &mut BytesMut);
    fn decode(flags: u8, src: &mut Bytes) -> Result<Self, DecodeError>;
}

/// Serializes a single [`CommandCodec`] value into a complete wire frame.
pub fn wire_frame<C: CommandCodec>(command: &C) -> Bytes {
    let mut payload = BytesMut::new();
    command.encode(&mut payload);
    let mut wire = BytesMut::with_capacity(FIXED_HEADER_MAX_LEN + payload.len());
    wire.put_u8((C::command() as u8) << COMMAND_SHIFT | (command.flags() & FLAGS_MASK));
    wire.put_varint(payload.len() as u32);
    wire.unsplit(payload);
    wire.freeze()
}
