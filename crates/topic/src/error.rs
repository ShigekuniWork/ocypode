use crate::{MAXIMUM_TOPIC_LAYER, MAXIMUM_TOPIC_LENGTH};

/// Error produced when constructing or decoding a topic fails.
#[derive(Debug, thiserror::Error)]
pub enum TopicError {
    #[error("topic length {length} exceeds the maximum of {MAXIMUM_TOPIC_LENGTH} bytes")]
    ExceedsMaxLength { length: usize },

    #[error("topic layer count {count} exceeds the maximum of {MAXIMUM_TOPIC_LAYER}")]
    ExceedsMaxLayerCount { count: usize },

    #[error("topic must not begin with a slash")]
    LeadingSlash,

    #[error("topic must not end with a slash")]
    TrailingSlash,

    #[error("topic must not contain consecutive slashes (empty layer)")]
    EmptyLayer,

    #[error("wildcard characters are not allowed in a publish topic")]
    WildcardInPublishTopic,

    #[error("the multi-layer wildcard '#' must appear only in terminal position")]
    MultiLayerWildcardNotTerminal,

    #[error(transparent)]
    Wire(#[from] wire::WireError),
}
