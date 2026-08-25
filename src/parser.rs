use std::{net::SocketAddr, str::Matches};

use crate::{expr::{Expr, IntermediateExpr::{self, Identifier}, Num}, lexer::token::{self, Keyword, Token}, parser::error::ParserError};

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

        println!("Ast: {:#?}", self.ast);

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
            // Simple Stuff
            Some(Token::Number(str)) => {
                let num: Num = Num::parse(str.as_str()).unwrap(); // can never fail

                self.skip(Ok(IntermediateExpr::Num(num)))
            }
            Some(Token::Keyword(Keyword::True)) => self.skip(Ok(IntermediateExpr::Bool(true))),
            Some(Token::Keyword(Keyword::False)) => self.skip(Ok(IntermediateExpr::Bool(false))),
            Some(Token::Keyword(Keyword::Nil)) => self.skip(Ok(IntermediateExpr::Nil)),
            Some(Token::Identifier(str)) => self.skip(Ok(IntermediateExpr::Identifier(str.clone()))), // cloned identifier, atleast for now

            // Assignments
            Some(Token::Keyword(Keyword::Local)) => self.parse_local_assignment(),

            Some(token) => Err(ParserError::Unimplemented(token.clone())),
            None => Err(ParserError::ReachedEnd)
        }
    }

    fn parse_local_assignment(&mut self) -> Result<IntermediateExpr, ParserError> {
        self.next(); // consume local

        self.parse_sequence(&[
            (
                &[Token::Identifier("".to_string()), Token::Eq], 
                |parser| {
                    let content = parser.parse_bp(0)?;

                    Ok(IntermediateExpr::Assignment {
                        local: true,
                        expr: Box::new(content)
                    })
                }, 
                Some(|_| ParserError::InvalidLocal)
            ),
        ])
        
    }

    fn parse_sequence<F>(
        &mut self,
        sequences: &[(&[Token], F, Option<fn(&mut Parser) -> ParserError>)],
    ) -> Result<IntermediateExpr, ParserError>
    where
        F: Fn(&mut Parser) -> Result<IntermediateExpr, ParserError>
    {
        for (sequence, parse, fail) in sequences {
            let matches = sequence.iter().enumerate().all(|(offset, expected)| {
                self.tokens
                    .get(self.index + offset)
                    .is_some_and(|actual| actual.same_variant(expected))
            });


            if matches {
                self.index += sequence.len();
                return parse(self);
            } else if let Some(fail) = fail {
                return Err(fail(self))
            }
        }

        Err(ParserError::InvalidSequence)
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

    fn skip(&mut self, expr: Result<IntermediateExpr, ParserError>) -> Result<IntermediateExpr, ParserError> {
        self.next();
        expr
    } 
}