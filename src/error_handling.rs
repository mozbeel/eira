use crate::{init::error::InitError, lexer::error::LexerError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("initialization failed: {0}")]
    InitError(#[from] InitError),

    #[error("lexing failed: {0}")]
    LexerError(#[from] LexerError),
}