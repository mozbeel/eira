use crate::{error_handling::Error, init::Init, lexer::Lexer};

mod init;
mod lexer;
mod error_handling;

pub fn run() {
    if let Err(err) = prun() {
        eprintln!("{err}");
    }
}

fn prun() -> Result<(), Error> {
    let init = Init::new().run()?;
    let lexer = Lexer::new(&init).run()?;

    Ok(())
}