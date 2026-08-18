use crate::{error_handling::Error, init::Init};

mod init;
mod error_handling;

pub fn run() {
    if let Err(err) = prun() {
        eprintln!("{err}");
    }
}

fn prun() -> Result<(), Error> {
    let init = Init::new().run()?;

    Ok(())
}