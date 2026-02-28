//! Topic and topic filter types for the Ocypode messaging system.
//!
//! - [`Topic`] represents a fully specified topic used when publishing.
//! - [`TopicFilter`] represents a topic pattern that may contain wildcards,
//!   used when subscribing.

mod error;

use bytes::{Bytes, BytesMut};
pub use error::TopicError;

const MAXIMUM_TOPIC_LENGTH: usize = 256;
const MAXIMUM_TOPIC_LAYER: usize = 8;
const MAX_SLASH_COUNT: usize = MAXIMUM_TOPIC_LAYER - 1;

/// Byte used to separate topic layers.
const LAYER_SEPARATOR: u8 = b'/';
/// Single-layer wildcard character.
const SINGLE_LAYER_WILDCARD: u8 = b'+';
/// Multi-layer wildcard character.
const MULTI_LAYER_WILDCARD: u8 = b'#';

mod topic_wire;

use topic_wire::TopicWire;

/// A fully specified topic used when publishing messages.
///
/// Wildcards are not permitted.
pub struct Topic {
    wire: TopicWire,
}

impl Topic {
    /// Reads a [`Topic`] from a wire buffer.
    ///
    /// Returns an error if the topic format is invalid or if wildcard
    /// characters are present.
    pub fn decode(src: &mut Bytes) -> Result<Self, TopicError> {
        let wire = TopicWire::decode(src)?;

        for &byte in wire.as_bytes().iter() {
            if byte == SINGLE_LAYER_WILDCARD || byte == MULTI_LAYER_WILDCARD {
                return Err(TopicError::WildcardInPublishTopic);
            }
        }

        Ok(Self { wire })
    }

    /// Writes the topic as a length-prefixed (u16) byte sequence.
    pub fn encode_to(&self, dst: &mut BytesMut) {
        self.wire.encode_to(dst);
    }

    /// Returns the raw bytes of the topic.
    pub fn as_bytes(&self) -> &Bytes {
        self.wire.as_bytes()
    }

    /// Returns the number of layers.
    pub fn layer_count(&self) -> u8 {
        self.wire.layer_count()
    }

    /// Returns the byte slice of the layer at the given index.
    pub fn layer(&self, index: u8) -> &[u8] {
        self.wire.layer(index)
    }
}

/// Indicates which wildcard characters a [`TopicFilter`] contains.
///
/// Used by the routing layer to select the appropriate matching strategy:
/// `None` topics can be looked up in O(1) via a hash map or Bloom filter,
/// while filters with wildcards require trie traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WildcardKind {
    /// No wildcards; the filter is an exact topic string.
    None,
    /// Contains one or more `+` wildcards but no `#`.
    SingleLayer,
    /// Contains a `#` wildcard (always in terminal position) but no `+`.
    MultiLayer,
    /// Contains both `+` and `#` wildcards.
    Both,
}

/// A topic pattern used when subscribing.
///
/// May contain single-layer (`+`) and multi-layer (`#`) wildcards as specified
/// in the topic specification. The `#` wildcard must appear only in terminal
/// position.
pub struct TopicFilter {
    wire: TopicWire,
    wildcard: WildcardKind,
}

impl TopicFilter {
    /// Reads a [`TopicFilter`] from a wire buffer.
    ///
    /// Returns an error if the topic format is invalid or if wildcard
    /// constraints are violated (e.g. `#` in a non-terminal position).
    pub fn decode(src: &mut Bytes) -> Result<Self, TopicError> {
        let wire = TopicWire::decode(src)?;
        let wildcard = Self::classify_wildcards(&wire)?;
        Ok(Self { wire, wildcard })
    }

    /// Writes the topic filter as a length-prefixed (u16) byte sequence.
    pub fn encode_to(&self, dst: &mut BytesMut) {
        self.wire.encode_to(dst);
    }

    /// Returns the raw bytes of the topic filter.
    pub fn as_bytes(&self) -> &Bytes {
        self.wire.as_bytes()
    }

    /// Returns the number of layers.
    pub fn layer_count(&self) -> u8 {
        self.wire.layer_count()
    }

    /// Returns the byte slice of the layer at the given index.
    pub fn layer(&self, index: u8) -> &[u8] {
        self.wire.layer(index)
    }

    /// Returns the wildcard kind for this filter.
    ///
    /// [`WildcardKind::None`] means the filter is an exact topic and can be
    /// matched in O(1) via a hash map or Bloom filter on the routing side.
    pub fn wildcard(&self) -> WildcardKind {
        self.wildcard
    }

    /// Scans layers for `+` and `#`, ensures `#` is only in the terminal layer,
    /// and returns the corresponding [`WildcardKind`].
    fn classify_wildcards(wire: &TopicWire) -> Result<WildcardKind, TopicError> {
        let layer_count = wire.layer_count();
        let mut has_single_layer = false;
        let mut has_multi_layer = false;

        for layer_index in 0..layer_count {
            let layer = wire.layer(layer_index);
            let is_terminal_layer = layer_index + 1 == layer_count;

            let layer_has_plus = layer.contains(&SINGLE_LAYER_WILDCARD);
            let layer_has_hash = layer.contains(&MULTI_LAYER_WILDCARD);

            if layer_has_plus {
                has_single_layer = true;
            }
            if layer_has_hash {
                if !is_terminal_layer {
                    return Err(TopicError::MultiLayerWildcardNotTerminal);
                }
                has_multi_layer = true;
            }
        }

        let wildcard = match (has_single_layer, has_multi_layer) {
            (false, false) => WildcardKind::None,
            (true, false) => WildcardKind::SingleLayer,
            (false, true) => WildcardKind::MultiLayer,
            (true, true) => WildcardKind::Both,
        };

        Ok(wildcard)
    }
}

#[cfg(test)]
mod tests {
    use bytes::BytesMut;
    use wire::WireEncode;

    use super::*;

    fn make_wire_bytes(topic: &[u8]) -> Bytes {
        let mut buf = BytesMut::new();
        buf.put_length_prefixed_u16(topic);
        buf.freeze()
    }

    // --- Topic ---

    #[test]
    fn topic_decode_simple() {
        let mut src = make_wire_bytes(b"sensor/data");
        let topic = Topic::decode(&mut src).unwrap();
        assert_eq!(topic.as_bytes(), &Bytes::from_static(b"sensor/data"));
        assert_eq!(topic.layer_count(), 2);
        assert_eq!(topic.layer(0), b"sensor");
        assert_eq!(topic.layer(1), b"data");
    }

    #[test]
    fn topic_decode_single_layer() {
        let mut src = make_wire_bytes(b"events");
        let topic = Topic::decode(&mut src).unwrap();
        assert_eq!(topic.layer_count(), 1);
        assert_eq!(topic.layer(0), b"events");
    }

    #[test]
    fn topic_decode_max_layers() {
        let mut src = make_wire_bytes(b"a/b/c/d/e/f/g/h");
        let topic = Topic::decode(&mut src).unwrap();
        assert_eq!(topic.layer_count(), 8);
    }

    #[test]
    fn topic_encode_roundtrip() {
        let mut src = make_wire_bytes(b"sensor/data/temperature");
        let topic = Topic::decode(&mut src).unwrap();
        let mut dst = BytesMut::new();
        topic.encode_to(&mut dst);
        let mut roundtripped = dst.freeze();
        let decoded = Topic::decode(&mut roundtripped).unwrap();
        assert_eq!(decoded.as_bytes(), &Bytes::from_static(b"sensor/data/temperature"));
    }

    #[test]
    fn topic_rejects_leading_slash() {
        let mut src = make_wire_bytes(b"/sensor/data");
        assert!(matches!(Topic::decode(&mut src), Err(TopicError::LeadingSlash)));
    }

    #[test]
    fn topic_rejects_trailing_slash() {
        let mut src = make_wire_bytes(b"sensor/data/");
        assert!(matches!(Topic::decode(&mut src), Err(TopicError::TrailingSlash)));
    }

    #[test]
    fn topic_rejects_empty_layer() {
        let mut src = make_wire_bytes(b"sensor//data");
        assert!(matches!(Topic::decode(&mut src), Err(TopicError::EmptyLayer)));
    }

    #[test]
    fn topic_rejects_too_many_layers() {
        let mut src = make_wire_bytes(b"a/b/c/d/e/f/g/h/i");
        assert!(matches!(Topic::decode(&mut src), Err(TopicError::ExceedsMaxLayerCount { .. })));
    }

    #[test]
    fn topic_rejects_exceeds_max_length() {
        let long = vec![b'a'; 257];
        let mut src = make_wire_bytes(&long);
        assert!(matches!(Topic::decode(&mut src), Err(TopicError::ExceedsMaxLength { .. })));
    }

    #[test]
    fn topic_rejects_wildcard_plus() {
        let mut src = make_wire_bytes(b"sensor/+/data");
        assert!(matches!(Topic::decode(&mut src), Err(TopicError::WildcardInPublishTopic)));
    }

    #[test]
    fn topic_rejects_wildcard_hash() {
        let mut src = make_wire_bytes(b"sensor/#");
        assert!(matches!(Topic::decode(&mut src), Err(TopicError::WildcardInPublishTopic)));
    }

    // --- TopicFilter ---

    #[test]
    fn topic_filter_exact_no_wildcard() {
        let mut src = make_wire_bytes(b"sensor/data");
        let filter = TopicFilter::decode(&mut src).unwrap();
        assert_eq!(filter.wildcard(), WildcardKind::None);
    }

    #[test]
    fn topic_filter_single_layer_wildcard() {
        let mut src = make_wire_bytes(b"sensor/+/data");
        let filter = TopicFilter::decode(&mut src).unwrap();
        assert_eq!(filter.wildcard(), WildcardKind::SingleLayer);
    }

    #[test]
    fn topic_filter_multi_layer_wildcard() {
        let mut src = make_wire_bytes(b"sensor/data/#");
        let filter = TopicFilter::decode(&mut src).unwrap();
        assert_eq!(filter.wildcard(), WildcardKind::MultiLayer);
        assert_eq!(filter.layer_count(), 3);
    }

    #[test]
    fn topic_filter_both_wildcards() {
        let mut src = make_wire_bytes(b"sensor/+/data/#");
        let filter = TopicFilter::decode(&mut src).unwrap();
        assert_eq!(filter.wildcard(), WildcardKind::Both);
    }

    #[test]
    fn topic_filter_rejects_hash_non_terminal() {
        let mut src = make_wire_bytes(b"sensor/#/data");
        assert!(matches!(
            TopicFilter::decode(&mut src),
            Err(TopicError::MultiLayerWildcardNotTerminal)
        ));
    }

    #[test]
    fn topic_filter_encode_roundtrip() {
        let mut src = make_wire_bytes(b"sensor/+/#");
        let filter = TopicFilter::decode(&mut src).unwrap();
        let mut dst = BytesMut::new();
        filter.encode_to(&mut dst);
        let mut roundtripped = dst.freeze();
        let decoded = TopicFilter::decode(&mut roundtripped).unwrap();
        assert_eq!(decoded.as_bytes(), &Bytes::from_static(b"sensor/+/#"));
        assert_eq!(decoded.wildcard(), WildcardKind::Both);
    }
}
