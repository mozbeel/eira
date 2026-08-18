use crate::init::error::InitError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("initialization failed: {0}")]
    InitError(#[from] InitError)
}