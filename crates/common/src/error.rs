/// Error returned when reading a typed field from a wire buffer fails.
#[derive(Debug, thiserror::Error)]
pub enum WireError {
    #[error("buffer too short: expected at least {expected} bytes, got {actual}")]
    BufferTooShort { expected: usize, actual: usize },

    #[error("variable-length integer exceeds the maximum of 4 bytes")]
    VariableLengthOverflow,
}
