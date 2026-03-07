use std::fmt;

use bytes::Bytes;

/// Maximum number of topic layers (segments separated by `/`).
const MAX_LAYERS: usize = 8;

/// Maximum topic length in bytes.
const MAX_TOPIC_LENGTH: usize = 256;

/// Single-layer wildcard token.
const WILDCARD_SINGLE: &[u8] = b"+";

/// Multi-layer wildcard token.
const WILDCARD_MULTI: &[u8] = b"#";

/// Reserved system topic prefix.
const SYS_PREFIX: &[u8] = b"$SYS";

/// Errors returned when constructing or validating a [`Topic`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopicError {
    /// The topic byte string is empty.
    Empty,
    /// The topic exceeds the maximum allowed length.
    TooLong { len: usize },
    /// The topic has a leading slash.
    LeadingSlash,
    /// The topic has a trailing slash.
    TrailingSlash,
    /// The topic contains consecutive slashes (empty layer).
    EmptyLayer,
    /// The topic has more layers than the allowed maximum.
    TooManyLayers { count: usize },
    /// A multi-layer wildcard (`#`) was found outside the terminal position.
    MultiWildcardNotTerminal,
    /// A wildcard token (`+` or `#`) was found in a publish topic.
    WildcardInPublishTopic,
    /// A wildcard token is mixed with other characters in the same layer
    /// (e.g. `sensor+` or `data#1`).
    InvalidWildcardUsage,
    /// The first layer is `$SYS`, which is reserved for system use.
    ReservedSysPrefix,
}

impl fmt::Display for TopicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TopicError::Empty => write!(f, "topic must not be empty"),
            TopicError::TooLong { len } => {
                write!(f, "topic length {len} exceeds maximum {MAX_TOPIC_LENGTH}")
            }
            TopicError::LeadingSlash => write!(f, "topic must not have a leading slash"),
            TopicError::TrailingSlash => write!(f, "topic must not have a trailing slash"),
            TopicError::EmptyLayer => {
                write!(f, "topic must not contain consecutive slashes (empty layer)")
            }
            TopicError::TooManyLayers { count } => {
                write!(f, "topic has {count} layers, maximum is {MAX_LAYERS}")
            }
            TopicError::MultiWildcardNotTerminal => {
                write!(f, "multi-layer wildcard '#' must be in terminal position")
            }
            TopicError::WildcardInPublishTopic => {
                write!(f, "wildcards are not allowed in publish topics")
            }
            TopicError::InvalidWildcardUsage => {
                write!(f, "wildcard must occupy the entire layer by itself")
            }
            TopicError::ReservedSysPrefix => {
                write!(f, "topics starting with '$SYS' are reserved for system use")
            }
        }
    }
}

impl std::error::Error for TopicError {}

/// Whether a topic is used for publishing or subscribing.
/// Wildcards are only allowed when subscribing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopicKind {
    Publish,
    Subscribe,
}

/// A validated topic.
///
/// Use [`Topic::new`] or [`Topic::parse`] to construct an instance.
/// The raw bytes are stored as-is; [`segments`](Topic::segments) splits on `/`.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Topic(Bytes);

impl Topic {
    /// Create a topic from raw bytes **without** validation.
    ///
    /// Prefer [`Topic::parse`] when the input comes from an untrusted source.
    pub fn new(bytes: Bytes) -> Self {
        Topic(bytes)
    }

    /// Parse and validate raw bytes into a [`Topic`].
    pub fn parse(bytes: Bytes, kind: TopicKind) -> Result<Self, TopicError> {
        validate(&bytes, kind)?;
        Ok(Topic(bytes))
    }

    /// Return the raw bytes of the topic.
    #[allow(dead_code)]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Return an iterator over the segments (layers) of this topic.
    /// Segments are separated by `/`. Empty segments are not produced because
    /// validation rejects leading/trailing/consecutive slashes.
    pub fn segments(&self) -> impl Iterator<Item = &[u8]> + '_ {
        self.0.split(|&b| b == b'/').filter(|s| !s.is_empty())
    }

    /// Return the number of layers (segments) in this topic.
    #[allow(dead_code)]
    pub fn layer_count(&self) -> usize {
        self.segments().count()
    }

    /// Returns `true` if this topic contains any wildcard tokens.
    #[allow(dead_code)]
    pub fn has_wildcards(&self) -> bool {
        self.segments().any(|s| s == WILDCARD_SINGLE || s == WILDCARD_MULTI)
    }

    /// Returns `true` if `publish_topic` matches against this (possibly
    /// wildcard-bearing) subscription topic.
    ///
    /// The `publish_topic` is expected to be a concrete topic (no wildcards).
    #[allow(dead_code)]
    pub fn matches(&self, publish_topic: &Topic) -> bool {
        topic_matches(self.segments(), publish_topic.segments())
    }
}

impl fmt::Debug for Topic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match std::str::from_utf8(&self.0) {
            Ok(s) => write!(f, "Topic({s:?})"),
            Err(_) => write!(f, "Topic({:?})", &self.0[..]),
        }
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
    fn from(b: Bytes) -> Self {
        Topic(b)
    }
}

impl From<&'static [u8]> for Topic {
    fn from(b: &'static [u8]) -> Self {
        Topic(Bytes::from_static(b))
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

fn validate(raw: &[u8], kind: TopicKind) -> Result<(), TopicError> {
    if raw.is_empty() {
        return Err(TopicError::Empty);
    }
    if raw.len() > MAX_TOPIC_LENGTH {
        return Err(TopicError::TooLong { len: raw.len() });
    }
    if raw[0] == b'/' {
        return Err(TopicError::LeadingSlash);
    }
    if raw[raw.len() - 1] == b'/' {
        return Err(TopicError::TrailingSlash);
    }

    let segments: Vec<&[u8]> = raw.split(|&b| b == b'/').collect();

    // Empty layer check (consecutive slashes produce an empty segment between them)
    for seg in &segments {
        if seg.is_empty() {
            return Err(TopicError::EmptyLayer);
        }
    }

    if segments.len() > MAX_LAYERS {
        return Err(TopicError::TooManyLayers { count: segments.len() });
    }

    // Reserved $SYS check
    if segments[0] == SYS_PREFIX {
        return Err(TopicError::ReservedSysPrefix);
    }

    // Wildcard validation
    for (i, seg) in segments.iter().enumerate() {
        let is_single = *seg == WILDCARD_SINGLE;
        let is_multi = *seg == WILDCARD_MULTI;

        if is_single || is_multi {
            if kind == TopicKind::Publish {
                return Err(TopicError::WildcardInPublishTopic);
            }
            if is_multi && i != segments.len() - 1 {
                return Err(TopicError::MultiWildcardNotTerminal);
            }
        } else {
            // Check that wildcards are not embedded within a segment (e.g. `sensor+`)
            let has_plus = seg.contains(&b'+');
            let has_hash = seg.contains(&b'#');
            if has_plus || has_hash {
                return Err(TopicError::InvalidWildcardUsage);
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Matching
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn topic_matches<'a>(
    sub_iter: impl Iterator<Item = &'a [u8]>,
    pub_iter: impl Iterator<Item = &'a [u8]>,
) -> bool {
    let sub_segs: Vec<&[u8]> = sub_iter.collect();
    let pub_segs: Vec<&[u8]> = pub_iter.collect();
    match_segments(&sub_segs, &pub_segs)
}

#[allow(dead_code)]
fn match_segments(sub: &[&[u8]], publish: &[&[u8]]) -> bool {
    let mut si = 0;
    let mut pi = 0;

    while si < sub.len() {
        let seg = sub[si];

        if seg == WILDCARD_MULTI {
            // '#' matches all remaining layers (zero or more)
            return true;
        }

        if pi >= publish.len() {
            // Publish topic exhausted but subscription still has segments
            return false;
        }

        if seg == WILDCARD_SINGLE {
            // '+' matches exactly one layer
            si += 1;
            pi += 1;
            continue;
        }

        if seg != publish[pi] {
            return false;
        }

        si += 1;
        pi += 1;
    }

    // Both must be exhausted for an exact match
    si == sub.len() && pi == publish.len()
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;

    // ---- helpers ----------------------------------------------------------

    fn topic(s: &'static str) -> Topic {
        Topic::new(Bytes::from_static(s.as_bytes()))
    }

    fn parse_pub(s: &str) -> Result<Topic, TopicError> {
        Topic::parse(Bytes::from(s.to_owned()), TopicKind::Publish)
    }

    fn parse_sub(s: &str) -> Result<Topic, TopicError> {
        Topic::parse(Bytes::from(s.to_owned()), TopicKind::Subscribe)
    }

    // ---- segments ---------------------------------------------------------

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
    fn layer_count_returns_correct_number() {
        assert_eq!(topic("a/b/c").layer_count(), 3);
        assert_eq!(topic("x").layer_count(), 1);
        assert_eq!(topic("a/b/c/d/e/f/g/h").layer_count(), 8);
    }

    // ---- validation: basic structure --------------------------------------

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
        // 256 bytes: "x" repeated 256 times, single layer
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

    // ---- validation: $SYS -------------------------------------------------

    #[test]
    fn parse_rejects_sys_prefix() {
        assert_eq!(parse_pub("$SYS/status"), Err(TopicError::ReservedSysPrefix));
        assert_eq!(parse_sub("$SYS/+"), Err(TopicError::ReservedSysPrefix));
    }

    #[test]
    fn parse_accepts_sys_not_at_first_layer() {
        // $SYS is only reserved when it is the *first* layer
        assert!(parse_pub("device/$SYS/info").is_ok());
    }

    // ---- validation: wildcards in publish ---------------------------------

    #[test]
    fn parse_rejects_single_wildcard_in_publish() {
        assert_eq!(parse_pub("sensor/+/data"), Err(TopicError::WildcardInPublishTopic));
    }

    #[test]
    fn parse_rejects_multi_wildcard_in_publish() {
        assert_eq!(parse_pub("sensor/#"), Err(TopicError::WildcardInPublishTopic));
    }

    // ---- validation: wildcards in subscribe -------------------------------

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

    // ---- has_wildcards ----------------------------------------------------

    #[test]
    fn has_wildcards_returns_true_for_single() {
        assert!(topic("sensor/+/data").has_wildcards());
    }

    #[test]
    fn has_wildcards_returns_true_for_multi() {
        assert!(topic("sensor/#").has_wildcards());
    }

    #[test]
    fn has_wildcards_returns_false_for_concrete() {
        assert!(!topic("sensor/data/temp").has_wildcards());
    }

    // ---- matching: exact --------------------------------------------------

    #[test]
    fn exact_topic_matches_itself() {
        let sub = topic("sensor/data/temp");
        let pub_t = topic("sensor/data/temp");
        assert!(sub.matches(&pub_t));
    }

    #[test]
    fn exact_topic_does_not_match_different_topic() {
        let sub = topic("sensor/data/temp");
        let pub_t = topic("sensor/data/humidity");
        assert!(!sub.matches(&pub_t));
    }

    #[test]
    fn exact_topic_does_not_match_prefix() {
        let sub = topic("sensor/data");
        let pub_t = topic("sensor/data/temp");
        assert!(!sub.matches(&pub_t));
    }

    #[test]
    fn exact_topic_does_not_match_shorter() {
        let sub = topic("sensor/data/temp");
        let pub_t = topic("sensor/data");
        assert!(!sub.matches(&pub_t));
    }

    // ---- matching: single-layer wildcard ----------------------------------

    #[test]
    fn single_wildcard_matches_one_layer() {
        let sub = topic("sensor/+/data");
        assert!(sub.matches(&topic("sensor/1/data")));
        assert!(sub.matches(&topic("sensor/2/data")));
        assert!(sub.matches(&topic("sensor/abc/data")));
    }

    #[test]
    fn single_wildcard_does_not_match_zero_layers() {
        let sub = topic("sensor/+/data");
        assert!(!sub.matches(&topic("sensor/data")));
    }

    #[test]
    fn single_wildcard_does_not_match_multiple_layers() {
        let sub = topic("sensor/+/data");
        assert!(!sub.matches(&topic("sensor/1/2/data")));
    }

    #[test]
    fn single_wildcard_at_end() {
        let sub = topic("sensor/data/+");
        assert!(sub.matches(&topic("sensor/data/temp")));
        assert!(sub.matches(&topic("sensor/data/humidity")));
        assert!(!sub.matches(&topic("sensor/data")));
        assert!(!sub.matches(&topic("sensor/data/temp/extra")));
    }

    #[test]
    fn single_wildcard_at_beginning() {
        let sub = topic("+/data/temp");
        assert!(sub.matches(&topic("sensor/data/temp")));
        assert!(sub.matches(&topic("device/data/temp")));
        assert!(!sub.matches(&topic("data/temp")));
    }

    #[test]
    fn multiple_single_wildcards() {
        let sub = topic("+/+/data");
        assert!(sub.matches(&topic("a/b/data")));
        assert!(!sub.matches(&topic("a/data")));
        assert!(!sub.matches(&topic("a/b/c/data")));
    }

    // ---- matching: multi-layer wildcard -----------------------------------

    #[test]
    fn multi_wildcard_matches_remaining_layers() {
        let sub = topic("sensor/data/#");
        assert!(sub.matches(&topic("sensor/data/temp")));
        assert!(sub.matches(&topic("sensor/data/alert/warning")));
        assert!(sub.matches(&topic("sensor/data/a/b/c/d")));
    }

    #[test]
    fn multi_wildcard_matches_zero_remaining_layers() {
        // `#` can match zero or more remaining layers
        let sub = topic("sensor/data/#");
        assert!(sub.matches(&topic("sensor/data")));
    }

    #[test]
    fn multi_wildcard_standalone_matches_everything() {
        let sub = topic("#");
        assert!(sub.matches(&topic("sensor")));
        assert!(sub.matches(&topic("sensor/data")));
        assert!(sub.matches(&topic("a/b/c/d/e")));
    }

    #[test]
    fn multi_wildcard_does_not_match_wrong_prefix() {
        let sub = topic("sensor/data/#");
        assert!(!sub.matches(&topic("device/data/temp")));
        assert!(!sub.matches(&topic("sensor/info/temp")));
    }

    // ---- matching: combined wildcards -------------------------------------

    #[test]
    fn single_and_multi_wildcard_combined() {
        let sub = topic("+/data/#");
        assert!(sub.matches(&topic("sensor/data")));
        assert!(sub.matches(&topic("sensor/data/temp")));
        assert!(sub.matches(&topic("device/data/a/b")));
        assert!(!sub.matches(&topic("sensor/info/temp")));
    }

    // ---- Display & Debug --------------------------------------------------

    #[test]
    fn display_shows_topic_string() {
        let t = topic("sensor/data/temp");
        assert_eq!(format!("{t}"), "sensor/data/temp");
    }

    #[test]
    fn debug_shows_topic_string() {
        let t = topic("sensor/data/temp");
        assert_eq!(format!("{t:?}"), "Topic(\"sensor/data/temp\")");
    }
}
