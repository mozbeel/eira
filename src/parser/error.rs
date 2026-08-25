use crate::lexer::token::Token;


#[derive(Debug, thiserror::Error)]
pub enum ParserError {
    #[error("Unimplemented: {0:?}")]
    Unimplemented(Token),

    #[error("Reached End")]
    ReachedEnd
}