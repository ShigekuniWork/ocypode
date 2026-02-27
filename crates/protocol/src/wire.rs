use std::mem::size_of;

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::DecodeError;

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
    fn read_u32(&mut self) -> Result<u32, DecodeError>;
    /// Reads a 1-byte length prefix followed by that many bytes.
    fn read_length_prefixed_u8(&mut self) -> Result<Bytes, DecodeError>;
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

    fn read_u32(&mut self) -> Result<u32, DecodeError> {
        if self.remaining() < size_of::<u32>() {
            return Err(DecodeError::BufferTooShort {
                expected: size_of::<u32>(),
                actual: self.remaining(),
            });
        }
        Ok(self.get_u32())
    }

    fn read_length_prefixed_u8(&mut self) -> Result<Bytes, DecodeError> {
        let length = self.read_u8()? as usize;
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
    /// Writes a 1-byte length prefix followed by the given bytes.
    fn put_length_prefixed_u8(&mut self, bytes: impl AsRef<[u8]>);
}

impl WireEncode for BytesMut {
    fn put_length_prefixed_u8(&mut self, bytes: impl AsRef<[u8]>) {
        let bytes = bytes.as_ref();
        self.put_u8(bytes.len() as u8);
        self.put(bytes);
    }
}
