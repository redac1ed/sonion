use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("malformed request/status line: {0}")]
    MalformedLine(String),

    #[error("invalid header: {0}")]
    InvalidHeader(String),

    #[error("request line exceeds {max} bytes")]
    RequestLineTooLarge { max: usize },

    #[error("headers exceed {max} bytes")]
    HeadersTooLarge { max: usize },

    #[error("body exceeds {max} bytes")]
    BodyTooLarge { max: usize },

    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("invalid status code: {0}")]
    InvalidStatus(u16),

    #[error("invalid chunk: {0}")]
    InvalidChunk(String),

    #[error("unexpected end of input")]
    UnexpectedEof,
}
