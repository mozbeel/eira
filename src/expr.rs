use std::fmt;

use crate::lexer::token::Token;

// Before Resolving and "Lowering"
#[derive(Debug)]
pub enum IntermediateExpr {
    Binary {
        lhs: Box<IntermediateExpr>,
        op: Token,
        rhs: Box<IntermediateExpr>
    },

    Num(Num),
    Bool(bool),
    Nil,
    Identifier(String),

    Assignment {
        local: bool,

        expr: Box<IntermediateExpr>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Num(pub u64);

impl Num {
    pub fn parse(s: &str) -> Result<Self, std::num::ParseFloatError> {
        let f = s.parse::<f64>()?;
        Ok(Self(f.to_bits()))
    }
} 

impl fmt::Debug for Num {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&f64::from_bits(self.0), f)
    }
}


// After Resolving
pub enum Expr {
    
}