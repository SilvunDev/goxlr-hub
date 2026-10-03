use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProtocolError {
    #[error("expected at least {expected} bytes, got {actual}")]
    TooShort { expected: usize, actual: usize },
    #[error("the header announces a body of {declared} bytes, got {actual}")]
    LengthMismatch { declared: usize, actual: usize },
    #[error("unknown command {0:#x}")]
    UnknownCommand(u32),
    #[error("invalid command body: {0}")]
    InvalidBody(&'static str),
}
