use thiserror::Error;

#[derive(Error, Debug)]
pub enum KnowlixError {
    #[error("Project not found: {0}")]
    ProjectNotFound(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Unsupported file type: {0}")]
    UnsupportedFileType(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Duplicate: {0}")]
    Duplicate(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Index error: {0}")]
    Index(String),

    #[error("AI not configured")]
    AiNotConfigured,

    #[error("AI provider unreachable: {0}")]
    AiProviderUnreachable(String),

    #[error("AI error: {0}")]
    Ai(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, KnowlixError>;
