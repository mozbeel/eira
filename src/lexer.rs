use std::{convert::identity, iter::Peekable, str::Chars};

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
            Some(c) if c.is_ascii_alphabetic() || *c == '_' => Some(Ok(self.lex_ascii_alphabetic())),
            Some(c) if Lexer::is_number(*c) => Some(self.lex_ascii_numeric(false)),
            Some(c) if c.is_whitespace() => self.skip(None),
            Some('+') => self.skip(Some(Ok(Token::Plus))),
            Some('-') => Some(Ok(self.lex_minus())),
            Some('*') => self.skip(Some(Ok(Token::Star))),
            Some('/') => Some(self.lex_sequence(&[
                ("//", Token::DoubleSlash),
                ("/", Token::Slash)
            ])),
            Some('%') => self.skip(Some(Ok(Token::Modulo))),
            Some('^') => self.skip(Some(Ok(Token::Caret))),
            Some('#') => self.skip(Some(Ok(Token::Hash))),
            Some('&') => self.skip(Some(Ok(Token::And))),
            Some('~') => Some(self.lex_sequence(&[
                ("~=", Token::TildeEq),
                ("~", Token::Tilde),
            ])),
            Some('|') => self.skip(Some(Ok(Token::Or))),
            Some('<') => Some(self.lex_sequence(&[
                ("<<", Token::ShiftLeft),
                ("<=", Token::SmallerEq),
                ("<", Token::Smaller),
            ])),
            Some('>') => Some(self.lex_sequence(&[
                (">>", Token::ShiftRight),
                (">=", Token::GreaterEq),
                (">", Token::Greater),
            ])),
            Some('=') => Some(self.lex_sequence(&[
                ("==", Token::DoubleEq),
                ("=", Token::Eq),
            ])),
            Some('(') => self.skip(Some(Ok(Token::ParenthOpen))),
            Some(')') => self.skip(Some(Ok(Token::ParenthClosed))),
            Some('{') => self.skip(Some(Ok(Token::CurlyBracketOpen))),
            Some('}') => self.skip(Some(Ok(Token::CurlyBracketClosed))),
            Some('[') => self.skip(Some(Ok(Token::SqBracketOpen))),
            Some(']') => self.skip(Some(Ok(Token::SqBracketClosed))),
            Some(':') => Some(self.lex_sequence(&[
                ("::", Token::DoubleColon),
                (":", Token::Colon),
            ])),
            Some(';') => self.skip(Some(Ok(Token::SemiColon))),
            Some(',') => self.skip(Some(Ok(Token::Comma))),
            Some('.') => Some(self.lex_dot()),
            Some('"') => Some(self.lex_string('"')),
            Some('\'') => Some(self.lex_string('\'')),
            None => Some(Err(LexerError::ReachedEnd)),
            Some(c) => Some(Err(LexerError::UnknownCharacter(*c))),
        }
    }

    fn lex_ascii_alphabetic(&mut self) -> Token {
        let mut identifier = String::new();

        identifier.push(*self.peek().unwrap());
        self.next();

        while let Some(char) = self.peek() {
            if char.is_ascii_alphanumeric() || *char == '_' {
                identifier.push(*char);
                self.next();
            } else {
                break;
            }
        }

        if let Ok(keyword) = identifier.parse::<Keyword>() {
            Token::Keyword(keyword)
        } else if identifier == "_" {
            Token::Underscore
        } else {
            Token::Identifier(identifier)
        }
    }

    fn lex_ascii_numeric(&mut self, is_dot_first: bool) -> Result<Token, LexerError> {
        let mut num = String::new();

        let mut dot_occured = is_dot_first;

        if dot_occured {
            num.push_str("0.");
        }

        while let Some(char) = self.peek() {
            if Lexer::is_number(*char) {
                num.push(*char);
                self.next();
            } else if *char == '.' {
                if dot_occured {
                    return Err(LexerError::UnexpectedDot);
                }

                dot_occured = true;


                num.push(*char);
                self.next();
            } else {
                break;
            }
        }

        Ok(Token::Number(num))
    }

    fn lex_dot(&mut self) -> Result<Token, LexerError> {
        self.next(); // consume .
        if let Some(c) = self.peek() {
            if Lexer::is_number(*c) {
                self.lex_ascii_numeric(true)
            } else if *c == '.' {
                // already consumed one dot
                self.lex_sequence(&[
                    ("..", Token::TripleDot),
                    (".", Token::DoubleDot)
                ])
            } else {
                Ok(Token::Dot)
            }
        } else {
            Ok(Token::Dot)
        }

        
    }

    fn lex_minus(&mut self) -> Token {
        self.next(); // consume -

        if let Some(c) = self.peek() {
            if *c == '-' {
                self.lex_comment()
            } else {
                Token::Minus
            }
        } else {
            Token::Minus
        }
    }

    fn lex_comment(&mut self) -> Token {
        self.next(); // consume 2nd -
        
        let mut comment = String::new();

        while !matches!(self.peek(), Some('\n') | None) {
            comment.push(*self.peek().unwrap());
            self.next();
        }
        self.next(); //consume \n if exists

        Token::Comment(comment)
    }

    fn lex_sequence(&mut self, sequences: &[(&str, Token)]) -> Result<Token, LexerError> {
        for (sequence, token) in sequences {
            let mut chars = self.content.clone();
            let mut matched = true;

            for expected in sequence.chars() {
                if chars.next() != Some(expected) {
                    matched = false;
                    break;
                }
            }

            if matched {
                // Consume the characters from the real iterator.
                for _ in sequence.chars() {
                    self.content.next();
                }

                // Copying is trivial as the token variants that are used for this function
                // are stateless 
                return Ok(token.clone());
            }
        }

        Err(LexerError::InvalidSequence)
    }

    fn lex_string(&mut self, string_terminator_char: char) -> Result<Token, LexerError> {
        self.next(); // consume "

        let mut string = String::new();

        while !matches!(self.peek(), Some(_)) && *self.peek().unwrap() == string_terminator_char {
            if let None = self.peek() {
                return Err(LexerError::ExpectedQuote);
            }

            string.push(*self.peek().unwrap());
            self.next();
        }

        self.next(); // consume "

        Ok(Token::String(string))
    }

    fn skip(&mut self, result: Option<Result<Token, LexerError>>) -> Option<Result<Token, LexerError>> {
        self.next();
        return result;
    }

    fn peek(&mut self) -> Option<&char> {
        self.content.peek()
    }


    fn next(&mut self) -> Option<char> {
        self.content.next()
    }

    fn is_number(c: char) -> bool {
        c >= '0' && c <= '9'
    }
}