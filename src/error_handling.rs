use crate::{init::error::InitError, lexer::error::LexerError, parser::{Parser, error::ParserError}};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("initialization failed: {0}")]
    InitError(#[from] InitError),

    #[error("lexing failed: {0}")]
    LexerError(#[from] LexerError),

    #[error("parser failed: {0}")]
    ParserError(#[from] ParserError),
}