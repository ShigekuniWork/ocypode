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
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
    #[error("Invalid size bytes: {0}")]
    InvalidSizeBytes(usize),
    #[error("Invalid version: {0}")]
    #[allow(dead_code)]
    InvalidVersion(String),
    #[error("IO error: {0}")]
    IoError(String),
}

impl From<io::Error> for CodecError {
    fn from(err: io::Error) -> Self {
        CodecError::IoError(err.to_string())
    }
}

impl From<prost::EncodeError> for CodecError {
    fn from(err: prost::EncodeError) -> Self {
        CodecError::InvalidFormat(err.to_string())
    }
}

impl From<prost::DecodeError> for CodecError {
    fn from(err: prost::DecodeError) -> Self {
        CodecError::InvalidFormat(err.to_string())
    }
}

#[derive(Debug, Error)]
pub enum ServerCodecError {
    #[error(transparent)]
    Codec(#[from] CodecError),
}

impl From<io::Error> for ServerCodecError {
    fn from(err: io::Error) -> Self {
        ServerCodecError::Codec(CodecError::from(err))
    }
}

impl From<prost::EncodeError> for ServerCodecError {
    fn from(err: prost::EncodeError) -> Self {
        ServerCodecError::Codec(CodecError::from(err))
    }
}

impl From<prost::DecodeError> for ServerCodecError {
    fn from(err: prost::DecodeError) -> Self {
        ServerCodecError::Codec(CodecError::from(err))
    }
}

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum ClientCodecError {
    #[error(transparent)]
    Codec(#[from] CodecError),
}

impl From<io::Error> for ClientCodecError {
    fn from(err: io::Error) -> Self {
        ClientCodecError::Codec(CodecError::from(err))
    }
}

impl From<prost::EncodeError> for ClientCodecError {
    fn from(err: prost::EncodeError) -> Self {
        ClientCodecError::Codec(CodecError::from(err))
    }
}

impl From<prost::DecodeError> for ClientCodecError {
    fn from(err: prost::DecodeError) -> Self {
        ClientCodecError::Codec(CodecError::from(err))
    }
}
