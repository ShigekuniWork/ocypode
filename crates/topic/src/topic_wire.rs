//! Internal wire representation of a topic string.
//!
//! Parses and stores a validated topic with pre-computed slash positions
//! so that individual layers can be accessed without re-scanning the raw bytes.

use bytes::{Bytes, BytesMut};
use common::wire::{WireDecode, WireEncode};

use crate::{
    LAYER_SEPARATOR, MAX_SLASH_COUNT, MAXIMUM_TOPIC_LAYER, MAXIMUM_TOPIC_LENGTH, TopicError,
};

/// Internal representation shared by [`crate::Topic`] and [`crate::TopicFilter`].
pub(crate) struct TopicWire {
    raw: Bytes,
    layer_count: u8,
    /// Byte positions of each `/` separator. Only the first `layer_count - 1`
    /// entries are valid.
    slash_positions: [u8; MAX_SLASH_COUNT],
}

impl TopicWire {
    /// Reads a length-prefixed (u16) topic from the wire and validates the
    /// basic topic format: length, layer count, leading/trailing slashes,
    /// and empty layers. Wildcard validation is left to the callers.
    pub(crate) fn decode(src: &mut Bytes) -> Result<Self, TopicError> {
        let raw = src.read_length_prefixed_u16()?;
        Self::validate_length(&raw)?;
        Self::validate_no_leading_or_trailing_slash(&raw)?;
        let (slash_positions, layer_count) = Self::scan_slashes(&raw)?;
        Ok(Self { raw, layer_count, slash_positions })
    }

    fn validate_length(raw: &[u8]) -> Result<(), TopicError> {
        if raw.is_empty() || raw.len() > MAXIMUM_TOPIC_LENGTH {
            return Err(TopicError::ExceedsMaxLength { length: raw.len() });
        }
        Ok(())
    }

    fn validate_no_leading_or_trailing_slash(raw: &[u8]) -> Result<(), TopicError> {
        if raw[0] == LAYER_SEPARATOR {
            return Err(TopicError::LeadingSlash);
        }
        if raw[raw.len() - 1] == LAYER_SEPARATOR {
            return Err(TopicError::TrailingSlash);
        }
        Ok(())
    }

    fn scan_slashes(raw: &[u8]) -> Result<([u8; MAX_SLASH_COUNT], u8), TopicError> {
        let mut slash_positions = [0u8; MAX_SLASH_COUNT];
        let mut slash_count: usize = 0;
        let mut prev_was_slash = false;

        for (i, &byte) in raw.iter().enumerate() {
            if byte == LAYER_SEPARATOR {
                if prev_was_slash {
                    return Err(TopicError::EmptyLayer);
                }
                if slash_count >= MAX_SLASH_COUNT {
                    let detected_layer_count = slash_count + 2;
                    return Err(TopicError::ExceedsMaxLayerCount { count: detected_layer_count });
                }
                slash_positions[slash_count] = i as u8;
                slash_count += 1;
                prev_was_slash = true;
            } else {
                prev_was_slash = false;
            }
        }

        let layer_count = slash_count + 1;
        if layer_count > MAXIMUM_TOPIC_LAYER {
            return Err(TopicError::ExceedsMaxLayerCount { count: layer_count });
        }

        Ok((slash_positions, layer_count as u8))
    }

    /// Writes the topic as a length-prefixed (u16) byte sequence.
    pub(crate) fn encode_to(&self, dst: &mut BytesMut) {
        dst.put_length_prefixed_u16(self.raw.as_ref());
    }

    /// Returns the raw bytes of the topic.
    pub(crate) fn as_bytes(&self) -> &Bytes {
        &self.raw
    }

    /// Returns the number of layers.
    pub(crate) fn layer_count(&self) -> u8 {
        self.layer_count
    }

    /// Returns the byte slice of the layer at the given index.
    ///
    /// # Panics
    ///
    /// Panics if `index >= layer_count`.
    pub(crate) fn layer(&self, index: u8) -> &[u8] {
        assert!(index < self.layer_count, "layer index out of bounds");
        let start =
            if index == 0 { 0 } else { self.slash_positions[(index - 1) as usize] as usize + 1 };
        let end = if index + 1 < self.layer_count {
            self.slash_positions[index as usize] as usize
        } else {
            self.raw.len()
        };
        &self.raw[start..end]
    }
}
