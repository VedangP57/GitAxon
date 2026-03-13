//! Stash operations.

use std::path::Path;

use git2::Repository;

use crate::errors::{GitfastError, GitfastResult};

fn open_repo(repo_path: &str) -> Result<Repository, GitfastError> {
    let path = Path::new(repo_path);
    if !path.exists() {
        return Err(GitfastError::RepoNotFound(repo_path.to_string()));
    }
    Repository::open(repo_path).map_err(|e| GitfastError::NotAGitRepo(e.to_string()))
}

/// Stash current changes with optional message.
pub async fn stash_push(repo_path: &str, message: Option<&str>) -> GitfastResult<()> {
    let path = repo_path.to_string();
    let msg = message.map(String::from);
    tokio::task::spawn_blocking(move || {
        let mut repo = open_repo(&path)?;
        let sig = repo
            .signature()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let msg_str = msg.as_deref().unwrap_or("stash");
        repo.stash_save(&sig, msg_str, None)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        Ok(())
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Pop the most recent stash (index 0).
pub async fn stash_pop(repo_path: &str) -> GitfastResult<()> {
    let path = repo_path.to_string();
    tokio::task::spawn_blocking(move || {
        let mut repo = open_repo(&path)?;
        repo.stash_pop(0, None)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        Ok(())
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}
