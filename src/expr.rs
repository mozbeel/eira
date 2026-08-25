use crate::lexer::token::Token;

// Before Resolving and "Lowering"
pub enum IntermediateExpr {
    Binary {
        lhs: Box<IntermediateExpr>,
        op: Token,
        rhs: Box<IntermediateExpr>
    },

    Num(Num)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Num(pub u64);

impl Num {
    pub fn parse(s: &str) -> Result<Self, std::num::ParseFloatError> {
        let f = s.parse::<f64>()?;
        Ok(Self(f.to_bits()))
    }
}


// After Resolving
pub enum Expr {
    
}