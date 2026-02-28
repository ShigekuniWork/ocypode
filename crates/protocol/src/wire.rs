use bytes::{Buf, BufMut, Bytes, BytesMut};
pub use wire::{WireDecode, WireEncode};

use crate::{Command, error::DecodeError};

pub(crate) const COMMAND_SHIFT: u8 = 4;
pub(crate) const FLAGS_MASK: u8 = 0x0F;
pub(crate) const FIXED_HEADER_MAX_LEN: usize = 5;

/// An opaque message payload, shared across PUB and MSG commands.
pub struct Payload(pub Bytes);

/// Key-value string headers shared across PUB and MSG commands.
///
/// Wire format per entry: `key_length` (u8) + `key` + `value_length` (u16) + `value`.
/// Entries are packed sequentially within the `header_size` bytes of the variable header.
///
/// Backed by a `Vec` rather than a `HashMap` for three reasons:
/// - Header counts are small in practice, so linear scan outperforms hash
///   lookup once hashing and collision overhead are accounted for.
/// - Insertion order is preserved, which matters for protocol-level ordering.
/// - Duplicate keys are permitted (e.g. multiple values under the same name).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Headers {
    entries: Vec<(Bytes, Bytes)>,
}

impl Headers {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a key-value entry. Duplicate keys are allowed.
    pub fn insert(&mut self, key: impl Into<Bytes>, value: impl Into<Bytes>) {
        self.entries.push((key.into(), value.into()));
    }

    /// Returns all entries in insertion order.
    pub fn entries(&self) -> &[(Bytes, Bytes)] {
        &self.entries
    }

    /// Serializes all entries into `dst` without a length prefix.
    /// The caller is responsible for writing the surrounding `header_size` field.
    pub(crate) fn encode_to(&self, dst: &mut BytesMut) {
        for (key, value) in &self.entries {
            dst.put_length_prefixed_u8(key.as_ref());
            dst.put_length_prefixed_u16(value.as_ref());
        }
    }

    /// Deserializes entries from `src` until it is fully consumed.
    /// `src` must be pre-sliced to exactly `header_size` bytes by the caller.
    pub(crate) fn decode_from(src: &mut Bytes) -> Result<Self, DecodeError> {
        let mut entries = Vec::new();
        while src.has_remaining() {
            let key = src.read_length_prefixed_u8()?;
            let value = src.read_length_prefixed_u16()?;
            entries.push((key, value));
        }
        Ok(Self { entries })
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
    let mut frame = BytesMut::with_capacity(FIXED_HEADER_MAX_LEN + payload.len());
    frame.put_u8((C::command() as u8) << COMMAND_SHIFT | (command.flags() & FLAGS_MASK));
    frame.put_varint(payload.len() as u32);
    frame.unsplit(payload);
    frame.freeze()
}
