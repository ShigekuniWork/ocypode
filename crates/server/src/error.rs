use std::io;

use thiserror::Error;

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
