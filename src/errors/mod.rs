//! Error types and handling.

use thiserror::Error;

/// Library-wide error type.
#[derive(Debug, Error)]
pub enum GitfastError {
    #[error("repository not found: {0}")]
    RepoNotFound(String),

    #[error("not a git repository: {0}")]
    NotAGitRepo(String),

    #[error("git operation failed: {0}")]
    GitOperationFailed(String),

    #[error("branch not found: {0}")]
    BranchNotFound(String),

    #[error("commit not found: {0}")]
    CommitNotFound(String),

    #[error("database error: {0}")]
    DatabaseError(String),

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("remote error: {0}")]
    RemoteError(String),

    #[error("authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

/// Result type alias for operations that can fail with [`GitfastError`].
pub type GitfastResult<T> = Result<T, GitfastError>;
