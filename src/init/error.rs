
#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("No Input File")]
    NoInput,

    #[error("Input File couldn't be found")]
    InputNotFound,

    #[error("failed to read file: {0}")]
    ReadFile(#[from] std::io::Error),
}