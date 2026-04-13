use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Internal processing error")]
    Internal,
    #[error("Unknown error occurred")]
    Unknown,
}
