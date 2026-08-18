use std::{iter::Peekable, str::Chars};

use crate::{init::Init, lexer::{error::LexerError, token::{Keyword, Token}}};

pub mod token;
pub mod error;

pub struct Lexer<'a> {
    content: Peekable<Chars<'a>>,
    tokens: Vec<Token>,
}

impl<'a> Lexer<'a> {
    pub fn new(init: &'a Init) -> Lexer<'a> {
        Lexer { content: init.content.chars().peekable(), tokens: vec![] }
    }

    pub fn run(mut self) -> Result<Vec<Token>, LexerError> {
        while self.peek().is_some() {
            let tok = self.lex_char();

            // skip if None
            if let Some(tok) = tok {
                self.tokens.push(tok?);
            }
        }

        println!("Tokens: {:#?}", self.tokens);


        Ok(self.tokens)
    }

    fn lex_char(&mut self) -> Option<Result<Token, LexerError>> {
        match self.peek() {
            Some(c) if c.is_ascii_alphabetic() => Some(self.lex_ascii()),
            Some(c) if c.is_whitespace() => { 
                self.next();
                None 
            },
            None => Some(Err(LexerError::ReachedEnd)),
            _ => Some(Err(LexerError::UnknownCharacter)),
        }
    }

    fn lex_ascii(&mut self) -> Result<Token, LexerError> {
        let mut identifier = String::new();

        while let Some(char) = self.peek() {
            if char.is_ascii_alphabetic() {
                identifier.push(*char);
                self.next();
            } else {
                break;
            }
        }

        if let Ok(keyword) = identifier.parse::<Keyword>() {
            Ok(Token::Keyword(keyword))
        } else {
            Ok(Token::Identifier(identifier))
        }
    }

    fn peek(&mut self) -> Option<&char> {
        self.content.peek()
    }

    fn next(&mut self) -> Option<char> {
        self.content.next()
    }
}