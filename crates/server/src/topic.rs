use std::fmt;

use bytes::{Bytes, BytesMut};

pub use crate::error::TopicError;

pub const MAX_LAYERS: usize = 8;
pub const MAX_TOPIC_LENGTH: usize = 256;

pub(crate) const WILDCARD_SINGLE: &[u8] = b"+";
const WILDCARD_SINGLE_BYTE: u8 = b'+';

pub(crate) const WILDCARD_MULTI: &[u8] = b"#";
const WILDCARD_MULTI_BYTE: u8 = b'#';

const SYS_PREFIX: &[u8] = b"$SYS";

/// Global topic prefix. Topics starting with `$G` are visible across all
/// tenants. When no tenants are configured the prefix is accepted but has
/// no additional effect.
pub const GLOBAL_PREFIX: &[u8] = b"$G";

const SEP_BYTE: u8 = b'/';

/// A validated publish topic. Wildcards are not allowed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Topic(Bytes);

impl Topic {
    pub fn new(bytes: BytesMut) -> Result<Self, TopicError> {
        let bytes = bytes.freeze();
        validate_segments(&bytes).and_then(|s| validate_no_wildcards(&s))?;
        Ok(Topic(bytes))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn segments(&self) -> impl Iterator<Item = &[u8]> + '_ {
        self.0.split(|&byte| byte == SEP_BYTE).filter(|s| !s.is_empty())
    }
}

impl fmt::Display for Topic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match std::str::from_utf8(&self.0) {
            Ok(s) => f.write_str(s),
            Err(_) => write!(f, "{:?}", &self.0[..]),
        }
    }
}

impl From<Bytes> for Topic {
    fn from(bytes: Bytes) -> Self {
        Topic(bytes)
    }
}

impl From<&'static [u8]> for Topic {
    fn from(bytes: &'static [u8]) -> Self {
        Topic(Bytes::from_static(bytes))
    }
}

/// A validated subscribe topic filter. Wildcards (`+`, `#`) are allowed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TopicFilter(Bytes);

impl TopicFilter {
    pub fn new(bytes: BytesMut) -> Result<Self, TopicError> {
        let bytes = bytes.freeze();
        validate_segments(&bytes).and_then(|s| validate_wildcard_placement(&s))?;
        Ok(TopicFilter(bytes))
    }

    #[allow(dead_code)]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn segments(&self) -> impl Iterator<Item = &[u8]> + '_ {
        self.0.split(|&byte| byte == SEP_BYTE).filter(|s| !s.is_empty())
    }
}

impl fmt::Display for TopicFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match std::str::from_utf8(&self.0) {
            Ok(s) => f.write_str(s),
            Err(_) => write!(f, "{:?}", &self.0[..]),
        }
    }
}

fn validate_raw(raw: &[u8]) -> Result<&[u8], TopicError> {
    if raw.is_empty() {
        return Err(TopicError::Empty);
    }
    if raw.len() > MAX_TOPIC_LENGTH {
        return Err(TopicError::TooLong { len: raw.len() });
    }
    if raw[0] == SEP_BYTE {
        return Err(TopicError::LeadingSlash);
    }
    if raw[raw.len() - 1] == SEP_BYTE {
        return Err(TopicError::TrailingSlash);
    }
    Ok(raw)
}

fn validate_segments(raw: &[u8]) -> Result<Vec<&[u8]>, TopicError> {
    let raw = validate_raw(raw)?;
    let segments: Vec<&[u8]> = raw.split(|&byte| byte == SEP_BYTE).collect();

    if segments.iter().any(|s| s.is_empty()) {
        return Err(TopicError::EmptyLayer);
    }
    if segments.len() > MAX_LAYERS {
        return Err(TopicError::TooManyLayers { count: segments.len() });
    }
    if segments[0] == SYS_PREFIX {
        return Err(TopicError::ReservedSysPrefix);
    }
    if segments[0] == GLOBAL_PREFIX && segments.len() < 2 {
        return Err(TopicError::GlobalPrefixWithoutTopic);
    }
    Ok(segments)
}

fn has_wildcard(seg: &[u8]) -> bool {
    seg.contains(&WILDCARD_SINGLE_BYTE) || seg.contains(&WILDCARD_MULTI_BYTE)
}

fn matchable_segments<'a>(segments: &'a [&'a [u8]]) -> &'a [&'a [u8]] {
    if segments[0] == GLOBAL_PREFIX { &segments[1..] } else { segments }
}

fn validate_no_wildcards(segments: &[&[u8]]) -> Result<(), TopicError> {
    matchable_segments(segments).iter().try_for_each(|seg| {
        if has_wildcard(seg) { Err(TopicError::WildcardInPublishTopic) } else { Ok(()) }
    })
}

fn validate_wildcard_placement(segments: &[&[u8]]) -> Result<(), TopicError> {
    let matchable = matchable_segments(segments);

    matchable.iter().enumerate().try_for_each(|(i, seg)| {
        if *seg == WILDCARD_SINGLE || *seg == WILDCARD_MULTI {
            if *seg == WILDCARD_MULTI && i != matchable.len() - 1 {
                return Err(TopicError::MultiWildcardNotTerminal);
            }
            Ok(())
        } else if has_wildcard(seg) {
            Err(TopicError::InvalidWildcardUsage)
        } else {
            Ok(())
        }
    })
}

#[cfg(test)]
mod tests {
    use bytes::BytesMut;

    use super::*;

    fn topic(s: &'static str) -> Topic {
        Topic::new(BytesMut::from(s)).unwrap()
    }

    fn filter(s: &'static str) -> TopicFilter {
        TopicFilter::new(BytesMut::from(s)).unwrap()
    }

    fn parse_pub(s: &str) -> Result<Topic, TopicError> {
        Topic::new(BytesMut::from(s))
    }

    fn parse_sub(s: &str) -> Result<TopicFilter, TopicError> {
        TopicFilter::new(BytesMut::from(s))
    }

    #[test]
    fn segments_splits_simple_path() {
        let t = topic("a/b/c");
        let segs: Vec<_> = t.segments().collect();
        assert_eq!(segs, vec![b"a".as_ref(), b"b", b"c"]);
    }

    #[test]
    fn segments_single_component() {
        let t = topic("single");
        let segs: Vec<_> = t.segments().collect();
        assert_eq!(segs, vec![b"single".as_ref()]);
    }

    #[test]
    fn parse_rejects_empty_topic() {
        assert_eq!(parse_pub(""), Err(TopicError::Empty));
    }

    #[test]
    fn parse_rejects_leading_slash() {
        assert_eq!(parse_pub("/a/b"), Err(TopicError::LeadingSlash));
    }

    #[test]
    fn parse_rejects_trailing_slash() {
        assert_eq!(parse_pub("a/b/"), Err(TopicError::TrailingSlash));
    }

    #[test]
    fn parse_rejects_consecutive_slashes() {
        assert_eq!(parse_pub("a//b"), Err(TopicError::EmptyLayer));
    }

    #[test]
    fn parse_rejects_topic_exceeding_max_length() {
        let long = "a".repeat(MAX_TOPIC_LENGTH + 1);
        assert_eq!(parse_pub(&long), Err(TopicError::TooLong { len: MAX_TOPIC_LENGTH + 1 }));
    }

    #[test]
    fn parse_accepts_topic_at_max_length() {
        let exactly_max = "x".repeat(MAX_TOPIC_LENGTH);
        assert!(parse_pub(&exactly_max).is_ok());
    }

    #[test]
    fn parse_rejects_more_than_8_layers() {
        assert_eq!(parse_pub("a/b/c/d/e/f/g/h/i"), Err(TopicError::TooManyLayers { count: 9 }));
    }

    #[test]
    fn parse_accepts_exactly_8_layers() {
        assert!(parse_pub("a/b/c/d/e/f/g/h").is_ok());
    }

    #[test]
    fn parse_rejects_sys_prefix() {
        assert_eq!(parse_pub("$SYS/status"), Err(TopicError::ReservedSysPrefix));
        assert_eq!(parse_sub("$SYS/+"), Err(TopicError::ReservedSysPrefix));
    }

    #[test]
    fn parse_accepts_sys_not_at_first_layer() {
        assert!(parse_pub("device/$SYS/info").is_ok());
    }

    #[test]
    fn parse_accepts_global_prefix_publish() {
        assert!(parse_pub("$G/sensor/data").is_ok());
    }

    #[test]
    fn parse_accepts_global_prefix_subscribe_with_wildcard() {
        assert!(parse_sub("$G/sensor/+/data").is_ok());
        assert!(parse_sub("$G/sensor/#").is_ok());
    }

    #[test]
    fn parse_rejects_global_prefix_alone() {
        assert_eq!(parse_pub("$G"), Err(TopicError::GlobalPrefixWithoutTopic));
        assert_eq!(parse_sub("$G"), Err(TopicError::GlobalPrefixWithoutTopic));
    }

    #[test]
    fn is_global_returns_true_for_global_filter() {
        let f = filter("$G/broadcast/alerts");
        assert_eq!(f.segments().next(), Some(GLOBAL_PREFIX));
    }

    #[test]
    fn is_global_returns_false_for_normal_filter() {
        let f = filter("sensor/data");
        assert_ne!(f.segments().next(), Some(GLOBAL_PREFIX));
    }

    #[test]
    fn parse_rejects_single_wildcard_in_publish() {
        assert_eq!(parse_pub("sensor/+/data"), Err(TopicError::WildcardInPublishTopic));
    }

    #[test]
    fn parse_rejects_multi_wildcard_in_publish() {
        assert_eq!(parse_pub("sensor/#"), Err(TopicError::WildcardInPublishTopic));
    }

    #[test]
    fn parse_accepts_single_wildcard_in_subscribe() {
        assert!(parse_sub("sensor/+/data").is_ok());
    }

    #[test]
    fn parse_accepts_multi_wildcard_terminal_in_subscribe() {
        assert!(parse_sub("sensor/data/#").is_ok());
    }

    #[test]
    fn parse_accepts_standalone_multi_wildcard() {
        assert!(parse_sub("#").is_ok());
    }

    #[test]
    fn parse_accepts_standalone_single_wildcard() {
        assert!(parse_sub("+").is_ok());
    }

    #[test]
    fn parse_accepts_multiple_single_wildcards() {
        assert!(parse_sub("+/+/data").is_ok());
    }

    #[test]
    fn parse_rejects_multi_wildcard_not_terminal() {
        assert_eq!(parse_sub("sensor/#/data"), Err(TopicError::MultiWildcardNotTerminal));
    }

    #[test]
    fn parse_rejects_embedded_wildcard_plus() {
        assert_eq!(parse_sub("sensor+/data"), Err(TopicError::InvalidWildcardUsage));
    }

    #[test]
    fn parse_rejects_embedded_wildcard_hash() {
        assert_eq!(parse_sub("sensor#/data"), Err(TopicError::InvalidWildcardUsage));
    }

    #[test]
    fn parse_rejects_embedded_wildcard_plus_suffix() {
        assert_eq!(parse_sub("sensor/data+"), Err(TopicError::InvalidWildcardUsage));
    }

    #[test]
    fn display_shows_topic_string() {
        let t = topic("sensor/data/temp");
        assert_eq!(format!("{t}"), "sensor/data/temp");
    }

    #[test]
    fn filter_segments_splits_simple_path() {
        let f = filter("a/b/c");
        let segs: Vec<_> = f.segments().collect();
        assert_eq!(segs, vec![b"a".as_ref(), b"b", b"c"]);
    }

    #[test]
    fn filter_display_shows_filter_string() {
        let f = filter("sensor/+/temp");
        assert_eq!(format!("{f}"), "sensor/+/temp");
    }
}
