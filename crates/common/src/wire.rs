//! Wire-protocol primitives shared across Ocypode crates.
//!
//! Provides low-level encode/decode traits and the associated error type used
//! when reading typed fields from a raw byte buffer.

use std::mem::size_of;

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::WireError;

pub const VARINT_DATA_MASK: u8 = 0x7F;
pub const VARINT_CONTINUATION_BIT: u8 = 0x80;
pub const VARINT_DATA_BITS: u32 = 7;
pub const VARINT_MAX_BYTES: usize = 4;
/// Extension trait on [`Bytes`] for reading typed wire-protocol fields.
pub trait WireDecode {
    fn read_u8(&mut self) -> Result<u8, WireError>;
    fn read_u16(&mut self) -> Result<u16, WireError>;
    fn read_u32(&mut self) -> Result<u32, WireError>;
    /// Reads a variable-length integer (1–4 bytes, MSB = continuation flag).
    fn read_varint(&mut self) -> Result<u32, WireError>;
    /// Reads a 1-byte length prefix followed by that many bytes.
    fn read_length_prefixed_u8(&mut self) -> Result<Bytes, WireError>;
    /// Reads a 2-byte length prefix followed by that many bytes.
    fn read_length_prefixed_u16(&mut self) -> Result<Bytes, WireError>;
}

impl WireDecode for Bytes {
    fn read_u8(&mut self) -> Result<u8, WireError> {
        if self.remaining() < size_of::<u8>() {
            return Err(WireError::BufferTooShort {
                expected: size_of::<u8>(),
                actual: self.remaining(),
            });
        }
        Ok(self.get_u8())
    }

    fn read_u16(&mut self) -> Result<u16, WireError> {
        if self.remaining() < size_of::<u16>() {
            return Err(WireError::BufferTooShort {
                expected: size_of::<u16>(),
                actual: self.remaining(),
            });
        }
        Ok(self.get_u16())
    }

    fn read_u32(&mut self) -> Result<u32, WireError> {
        if self.remaining() < size_of::<u32>() {
            return Err(WireError::BufferTooShort {
                expected: size_of::<u32>(),
                actual: self.remaining(),
            });
        }
        Ok(self.get_u32())
    }

    fn read_varint(&mut self) -> Result<u32, WireError> {
        let mut value: u32 = 0;
        let mut shift = 0u32;
        for _ in 0..VARINT_MAX_BYTES {
            if !self.has_remaining() {
                return Err(WireError::BufferTooShort { expected: 1, actual: 0 });
            }
            let byte = self.get_u8();
            value |= ((byte & VARINT_DATA_MASK) as u32) << shift;
            if byte & VARINT_CONTINUATION_BIT == 0 {
                return Ok(value);
            }
            shift += VARINT_DATA_BITS;
        }
        Err(WireError::VariableLengthOverflow)
    }

    fn read_length_prefixed_u8(&mut self) -> Result<Bytes, WireError> {
        let length = self.read_u8()? as usize;
        if self.remaining() < length {
            return Err(WireError::BufferTooShort { expected: length, actual: self.remaining() });
        }
        Ok(self.copy_to_bytes(length))
    }

    fn read_length_prefixed_u16(&mut self) -> Result<Bytes, WireError> {
        let length = self.read_u16()? as usize;
        if self.remaining() < length {
            return Err(WireError::BufferTooShort { expected: length, actual: self.remaining() });
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
