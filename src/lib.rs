use crate::{error_handling::Error, init::Init, lexer::Lexer, parser::Parser};

mod init;
mod lexer;
mod parser;
mod error_handling;
mod expr;

pub fn run() {
    if let Err(err) = prun() {
        eprintln!("{err}");
    }
}

fn prun() -> Result<(), Error> {
    let init = Init::new().run()?;
    let tokens = Lexer::new(&init).run()?;
    let ast = Parser::new(&tokens).run()?;

    Ok(())
}