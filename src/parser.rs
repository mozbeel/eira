use crate::{expr::{IntermediateExpr, Num}, lexer::token::{self, Keyword, Token}, parser::error::ParserError};

pub mod error;

pub struct Parser<'a> {
    ast: Vec<IntermediateExpr>,
    tokens: &'a [Token],
    index: usize,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token]) -> Self {
        Parser { ast: vec![], tokens, index: 0 }
    }

    pub fn run(mut self) -> Result<Vec<IntermediateExpr>, ParserError> {
        while self.peek().is_some()  {
            let expr = self.parse_bp(0)?;

            self.ast.push(expr);
        }

        Ok(self.ast)
    }

    fn parse_bp(&mut self, min_bp: usize) -> Result<IntermediateExpr, ParserError> {
        let mut lhs = self.parse_prefix()?;
        
        // Postfix loop
        while self.peek().is_some() {
            let Some(expr) = self.parse_postfix(&lhs) else {
                break;
            };

            lhs = expr;
        }

        while let Some(op) = self.peek() {
            let op = op.clone();

            let Some((l_bp, r_bp)) = self.infix_bp(&op) else {
                break;
            };

            if l_bp < min_bp {
                break;
            }

            self.next();

            let rhs = self.parse_bp(r_bp)?;

            lhs = IntermediateExpr::Binary {
                lhs: Box::new(lhs),
                op,
                rhs: Box::new(rhs),
            };
        }

        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<IntermediateExpr, ParserError> {
        match self.peek() {
            Some(Token::Number(str)) => {
                let num: Num = Num::parse(str.as_str()).unwrap();

                Ok(IntermediateExpr::Num(num))
            }
            Some(token) => Err(ParserError::Unimplemented(token.clone())),
            None => Err(ParserError::ReachedEnd)
        }
    }

    fn parse_postfix(&mut self, lhs: &IntermediateExpr) -> Option<IntermediateExpr> {
        let tok = self.peek()?;

        // Todo: Fill this up
        match tok {
            _ => Option::None
        }
    }

    fn infix_bp(&self, op: &Token) -> Option<(usize, usize)> {
        use Token::*;

        match op {
            Caret => Some((21, 20)),
            Star | Slash | DoubleSlash | Modulo => Some((18, 19)), // * / // %
            Plus | Minus => Some((16, 17)), // + -
            DoubleDot => Some((15, 15)), // ..
            ShiftLeft | ShiftRight => Some((13, 14)), // << >>
            And => Some((11, 12)), // &
            Tilde => Some((9, 10)), // ~
            Or => Some((7, 8)), // or
            Smaller | Greater |  SmallerEq | GreaterEq | DoubleEq | TildeEq => Some((5, 6)), // < > <= >= == ~=
            Keyword(token::Keyword::And) => Some((3, 4)), // and
            Keyword(token::Keyword::Or) => Some((1, 2)), // or
            _ => Option::None,
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn next(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.index);
        self.index += 1;
        token
    }
}