use crate::{expr::IntermediateExpr, lexer::token::Token};


#[derive(Debug, thiserror::Error)]
pub enum ParserError {
    #[error("Unimplemented: {0:?}")]
    Unimplemented(Token),

    #[error("Unimplemented Func: {0:?}")]
    UnimplementedFunc(&'static str),

    #[error("Reached End")]
    ReachedEnd,

    #[error("Exptected assignment: {0:?}")]
    ExptectedAssignment(IntermediateExpr),

    #[error("Invalid Sequence")]
    InvalidSequence,

    #[error("Invalid Local Usage")]
    InvalidLocal,
}