#[derive(Debug)]
pub enum Token {
    Identifier(String),
    Keyword(Keyword),
}

#[derive(Debug)]
pub enum Keyword {
    And,
    Break,
    Do,
    Else,
    ElseIf,
    End,
    False,
    For,
    Function,
    Global,
    Goto,
    If,
    In,
    Local,
    Nil,
    Not,
    Or,
    Repeat,
    Return,
    Then,
    True,
    Until,
    While
}

impl std::str::FromStr for Keyword {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "and" => Ok(Self::And),
            "break" => Ok(Self::Break),
            "do" => Ok(Self::Do),
            "else" => Ok(Self::Else),
            "elseif" => Ok(Self::ElseIf),
            "end" => Ok(Self::End),
            "false" => Ok(Self::False),
            "for" => Ok(Self::For),
            "function" => Ok(Self::Function),
            "global" => Ok(Self::Global),
            "goto" => Ok(Self::Goto),
            "if" => Ok(Self::If),
            "in" => Ok(Self::In),
            "local" => Ok(Self::Local),
            "nil" => Ok(Self::Nil),
            "not" => Ok(Self::Not),
            "or" => Ok(Self::Or),
            "repeat" => Ok(Self::Repeat),
            "return" => Ok(Self::Return),
            "then" => Ok(Self::Then),
            "true" => Ok(Self::True),
            "until" => Ok(Self::Until),
            "while" => Ok(Self::While),
            _ => Err(()),
        }
    }
}