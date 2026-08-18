
#[derive(Debug, thiserror::Error)]
pub enum LexerError {
    #[error("Unknown Character: {0}")]
    UnknownCharacter(char),

    #[error("Reached End Of File")]
    ReachedEnd,

    #[error("Invalid Sequence")]
    InvalidSequence,

    #[error("Missing Closing String Quote")]
    ExpectedQuote,

    #[error("Unexpected Dot")]
    UnexpectedDot,
}