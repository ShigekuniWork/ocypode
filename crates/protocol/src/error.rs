use crate::Command;

/// Error returned when decoding a wire frame fails.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("buffer too short: expected at least {expected} bytes, got {actual}")]
    BufferTooShort { expected: usize, actual: usize },

    #[error("variable-length integer exceeds the maximum of 4 bytes")]
    VariableLengthOverflow,

    #[error("unknown command byte: {0:#x}")]
    UnknownCommand(u8),

    #[error("unsupported command: {0:?}")]
    UnsupportedCommand(Command),

    #[error("unknown auth type: {0:#x}")]
    UnknownAuthType(u8),
}

/// Error returned when encoding a message fails.
#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error("message cannot be encoded in this direction")]
    WrongDirection,
}
