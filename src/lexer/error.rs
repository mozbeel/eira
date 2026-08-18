
#[derive(Debug, thiserror::Error)]
pub enum LexerError {
    #[error("Unknown Character")]
    UnknownCharacter,

    #[error("Reached End Of File")]
    ReachedEnd,
}