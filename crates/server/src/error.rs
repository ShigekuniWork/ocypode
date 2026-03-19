use std::io;

use thiserror::Error;

#[derive(Debug, PartialEq, Eq, Error)]
pub enum TopicError {
    #[error("topic is empty")]
    Empty,
    #[error("topic is too long: {len} bytes")]
    TooLong { len: usize },
    #[error("topic has a leading slash")]
    LeadingSlash,
    #[error("topic has a trailing slash")]
    TrailingSlash,
    #[error("topic contains an empty layer (consecutive slashes)")]
    EmptyLayer,
    #[error("topic has too many layers: {count}")]
    TooManyLayers { count: usize },
    #[error("topic starts with the reserved $SYS prefix")]
    ReservedSysPrefix,
    #[error("$G prefix must be followed by at least one topic layer")]
    GlobalPrefixWithoutTopic,
    #[error("wildcards are not allowed in publish topics")]
    WildcardInPublishTopic,
    #[error("multi-level wildcard '#' must be the last segment")]
    MultiWildcardNotTerminal,
    #[error("wildcard characters must occupy an entire segment")]
    InvalidWildcardUsage,
}

#[derive(Debug, Error)]
pub enum CodecError {
    #[error("Error")]
    #[allow(dead_code)]
    Error,
    #[error("Invalid command")]
    #[allow(dead_code)]
    InvalidCommand,
    #[error("Encode error: {0}")]
    Encode(#[from] prost::EncodeError),
    #[error("Decode error: {0}")]
    Decode(#[from] prost::DecodeError),
    #[error("Invalid size bytes: {0}")]
    InvalidSizeBytes(usize),
    #[error("Invalid version: {0}")]
    #[allow(dead_code)]
    InvalidVersion(String),
}

#[derive(Debug, Error)]
pub enum ServerCodecError {
    #[error(transparent)]
    Codec(#[from] CodecError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum ClientCodecError {
    #[error(transparent)]
    Codec(#[from] CodecError),
    #[error(transparent)]
    Io(#[from] io::Error),
}
