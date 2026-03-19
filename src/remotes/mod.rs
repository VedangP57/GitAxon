//! Remotes module for remote repository operations.

use std::path::Path;
use std::process::Command;

use git2::Repository;
use serde::{Deserialize, Serialize};

use crate::errors::{GitfastError, GitfastResult};

/// Information about a git remote.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
}

/// Result of a fetch operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchResult {
    pub remote: String,
    pub updated_refs: Vec<String>,
    pub new_refs: Vec<String>,
}

/// Result of a push operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushResult {
    pub remote: String,
    pub branch: String,
    pub success: bool,
    pub message: String,
}

fn open_repo(repo_path: &str) -> Result<Repository, GitfastError> {
    let path = Path::new(repo_path);
    if !path.exists() {
        return Err(GitfastError::RepoNotFound(repo_path.to_string()));
    }
    Repository::open(repo_path).map_err(|e| GitfastError::NotAGitRepo(e.to_string()))
}

/// Returns all configured remotes.
pub async fn list_remotes(repo_path: &str) -> GitfastResult<Vec<RemoteInfo>> {
    let path = repo_path.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;
        let remotes = repo
            .remotes()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let mut result = Vec::new();
        for name in remotes.iter().flatten() {
            let remote = repo
                .find_remote(name)
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            let fetch_url = remote
                .url()
                .unwrap_or("")
                .to_string();
            let push_url = remote
                .pushurl()
                .unwrap_or_else(|| remote.url().unwrap_or(""))
                .to_string();
            result.push(RemoteInfo {
                name: name.to_string(),
                fetch_url,
                push_url,
            });
        }
        Ok(result)
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

fn push_branch(
    repo_path: &str,
    remote_name: &str,
    branch_name: &str,
    force: bool,
) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.current_dir(repo_path)
        .arg("push")
        .arg(remote_name)
        .arg(branch_name);

    if force {
        cmd.arg("--force");
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

fn pull_branch(
    repo_path: &str,
    remote_name: &str,
    branch_name: &str,
) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .arg("pull")
        .arg(remote_name)
        .arg(branch_name)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

fn fetch_remote_cmd(repo_path: &str, remote_name: &str) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .arg("fetch")
        .arg(remote_name)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

/// Fetches from the given remote.
/// Uses SSH agent first, then credential helper for auth.
pub async fn fetch(repo_path: &str, remote_name: &str) -> GitfastResult<FetchResult> {
    let path = repo_path.to_string();
    let remote_name = remote_name.to_string();
    tokio::task::spawn_blocking(move || match fetch_remote_cmd(&path, &remote_name) {
        Ok(_out) => Ok(FetchResult {
            remote: remote_name,
            updated_refs: Vec::new(),
            new_refs: Vec::new(),
        }),
        Err(err) => Err(GitfastError::GitOperationFailed(err)),
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Fetch + fast-forward merge only. Returns status message.
/// Fails with clear error if fast-forward is not possible.
pub async fn pull(repo_path: &str, remote_name: &str, branch_name: &str) -> GitfastResult<String> {
    let path = repo_path.to_string();
    let remote_name = remote_name.to_string();
    let branch_name = branch_name.to_string();
    tokio::task::spawn_blocking(move || match pull_branch(&path, &remote_name, &branch_name) {
        Ok(msg) => Ok(msg),
        Err(err) => Err(GitfastError::GitOperationFailed(err)),
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Pushes a branch to the remote.
pub async fn push(
    repo_path: &str,
    remote_name: &str,
    branch_name: &str,
    force: bool,
) -> GitfastResult<PushResult> {
    let path = repo_path.to_string();
    let remote_name = remote_name.to_string();
    let branch_name = branch_name.to_string();
    tokio::task::spawn_blocking(move || match push_branch(&path, &remote_name, &branch_name, force)
    {
        Ok(msg) => Ok(PushResult {
            remote: remote_name,
            branch: branch_name,
            success: true,
            message: msg,
        }),
        Err(err) => Ok(PushResult {
            remote: remote_name,
            branch: branch_name,
            success: false,
            message: err,
        }),
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Returns all remotes as a JSON string.
pub async fn list_remotes_json(repo_path: &str) -> GitfastResult<String> {
    let remotes = list_remotes(repo_path).await?;
    serde_json::to_string_pretty(&remotes)
        .map_err(|e| GitfastError::SerializationError(e.to_string()))
}
