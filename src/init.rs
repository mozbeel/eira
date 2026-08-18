pub mod error;

use std::{env, fs, path::Path};

use crate::init::error::InitError;

pub struct Init {
    pub content: String
}

impl Init {
    pub fn new() -> Self {
        return Init { content: "".to_string() }
    }

    pub fn run(mut self) -> Result<Self, InitError> {
        let args: Vec<_> = env::args().collect();

        if args.len() <= 1 {
            return Err(InitError::NoInput);
        }

        let input_path = Path::new(&args[1]);

        if !input_path.exists() {
            return Err(InitError::InputNotFound)
        }

        self.content = fs::read_to_string(&args[1])?; // ReadFile Error
        Ok(self)
    }
}
