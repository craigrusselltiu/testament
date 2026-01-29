use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TestamentError {
    #[error("No solution file found")]
    NoSolutionFound,

    #[error("Failed to read file: {path}")]
    FileRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse solution file: {0}")]
    SolutionParse(String),

    #[error("Failed to run dotnet: {0}")]
    DotnetExecution(String),

    #[error("Failed to parse TRX file: {0}")]
    TrxParse(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, TestamentError>;
