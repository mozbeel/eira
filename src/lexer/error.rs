
#[derive(Debug, thiserror::Error)]
pub enum LexerError {
    #[error("Unknown Character: {0}")]
    UnknownCharacter(char),

    #[error("Reached End Of File")]
    ReachedEnd,

    #[error("Invalid Sequence")]
    InvalidSequence,

    #[error("Expected {0}")]
    Expected(String),

    #[error("Unexpected {0}")]
    Unexpected(String),

    #[error("Invalid Amount of =, expected: {0}, got: {1}")]
    InvalidEqSequence(usize, usize)
}