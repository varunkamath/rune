use thiserror::Error;

#[derive(Error, Debug)]
pub enum RuneError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Indexing error: {0}")]
    Indexing(String),

    #[error("Search error: {0}")]
    Search(String),

    #[error("Parser error: {0}")]
    Parser(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Invalid query: {0}")]
    InvalidQuery(String),

    #[error("File too large: {0} bytes (max: {1} bytes)")]
    FileTooLarge(usize, usize),

    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Other error: {0}")]
    Other(String),
}

impl From<anyhow::Error> for RuneError {
    fn from(err: anyhow::Error) -> Self {
        RuneError::Other(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, RuneError>;
