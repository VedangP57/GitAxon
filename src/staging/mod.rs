//! Staging module for index and staging area operations.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use git2::{IndexAddOption, ObjectType, Repository, Status};
use serde::{Deserialize, Serialize};

use crate::diff::{get_staged_files_from_repo, get_unstaged_files_from_repo, DiffHunk};
use crate::errors::{GitfastError, GitfastResult};

/// Staging status of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum StagingStatus {
    Staged,
    Unstaged,
    Untracked,
    Conflicted,
    Ignored,
}

/// An index entry with staging status and optional diffs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    /// File path.
    pub path: String,
    /// Staging status.
    pub status: StagingStatus,
    /// Staged changes (index vs HEAD), if any.
    pub staged_diff: Option<Vec<DiffHunk>>,
    /// Unstaged changes (workdir vs index), if any.
    pub unstaged_diff: Option<Vec<DiffHunk>>,
}

fn status_to_staging_status(s: Status) -> StagingStatus {
    if s.is_conflicted() {
        return StagingStatus::Conflicted;
    }
    if s.is_ignored() {
        return StagingStatus::Ignored;
    }
    if s.is_wt_new() && !s.is_index_new() {
        return StagingStatus::Untracked;
    }
    if s.is_index_new()
        || s.is_index_modified()
        || s.is_index_deleted()
        || s.is_index_renamed()
        || s.is_index_typechange()
    {
        return StagingStatus::Staged;
    }
    StagingStatus::Unstaged
}

fn path_from_entry(entry: &git2::StatusEntry<'_>) -> Option<String> {
    entry
        .head_to_index()
        .and_then(|d| {
            d.new_file()
                .path()
                .or_else(|| d.old_file().path())
                .map(|p| p.to_string_lossy().into_owned())
        })
        .or_else(|| {
            entry.index_to_workdir().and_then(|d| {
                d.new_file()
                    .path()
                    .or_else(|| d.old_file().path())
                    .map(|p| p.to_string_lossy().into_owned())
            })
        })
}

/// Returns all files with their staging status.
pub async fn get_status(repo_path: &str) -> GitfastResult<Vec<IndexEntry>> {
    let path = repo_path.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true)
            .include_ignored(true)
            .renames_head_to_index(true)
            .renames_index_to_workdir(true);

        let statuses = repo
            .statuses(Some(&mut opts))
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        let staged_files = get_staged_files_from_repo(&repo).unwrap_or_default();
        let unstaged_files = get_unstaged_files_from_repo(&repo).unwrap_or_default();

        let mut staged_map: HashMap<String, Vec<DiffHunk>> = HashMap::new();
        for f in &staged_files {
            let path_key = f
                .new_path
                .clone()
                .or_else(|| f.old_path.clone())
                .unwrap_or_default();
            if !path_key.is_empty() {
                staged_map.insert(path_key, f.hunks.clone());
            }
        }

        let mut unstaged_map: HashMap<String, Vec<DiffHunk>> = HashMap::new();
        for f in &unstaged_files {
            let path_key = f
                .new_path
                .clone()
                .or_else(|| f.old_path.clone())
                .unwrap_or_default();
            if !path_key.is_empty() {
                unstaged_map.insert(path_key, f.hunks.clone());
            }
        }

        let mut entries = Vec::new();
        for entry in statuses.iter() {
            if let Some(path_str) = path_from_entry(&entry) {
                let status = status_to_staging_status(entry.status());
                let staged_diff = staged_map.remove(&path_str);
                let unstaged_diff = unstaged_map.remove(&path_str);

                entries.push(IndexEntry {
                    path: path_str,
                    status,
                    staged_diff: if staged_diff.as_ref().map(|h| h.is_empty()).unwrap_or(true) {
                        None
                    } else {
                        staged_diff
                    },
                    unstaged_diff: if unstaged_diff.as_ref().map(|h| h.is_empty()).unwrap_or(true) {
                        None
                    } else {
                        unstaged_diff
                    },
                });
            }
        }
        Ok(entries)
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

fn open_repo(repo_path: &str) -> Result<Repository, GitfastError> {
    let path = Path::new(repo_path);
    if !path.exists() {
        return Err(GitfastError::RepoNotFound(repo_path.to_string()));
    }
    Repository::open(repo_path).map_err(|e| GitfastError::NotAGitRepo(e.to_string()))
}

/// Stages an entire file.
pub async fn stage_file(repo_path: &str, file_path: &str) -> GitfastResult<()> {
    let path = repo_path.to_string();
    let file = file_path.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;
        let mut index = repo
            .index()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        index
            .add_path(Path::new(&file))
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        index
            .write()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        Ok(())
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Unstages a file (resets in index to HEAD).
pub async fn unstage_file(repo_path: &str, file_path: &str) -> GitfastResult<()> {
    let path = repo_path.to_string();
    let file = file_path.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;
        let head = repo
            .head()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let obj = head
            .peel(ObjectType::Commit)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        repo.reset_default(Some(&obj), [Path::new(&file)])
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        Ok(())
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Stages all modified and untracked files.
pub async fn stage_all(repo_path: &str) -> GitfastResult<()> {
    let path = repo_path.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;
        let mut index = repo
            .index()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        index
            .add_all(["*"].iter(), IndexAddOption::DEFAULT, None)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        index
            .write()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        Ok(())
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Unstages everything by doing a mixed reset to HEAD.
pub async fn unstage_all(repo_path: &str) -> GitfastResult<()> {
    let path = repo_path.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;
        let head = repo
            .head()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let obj = head
            .peel(ObjectType::Commit)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        // Mixed reset replaces index with HEAD's tree (unstages all)
        repo.reset(&obj, git2::ResetType::Mixed, None)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        Ok(())
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Creates a commit from the current index.
pub async fn create_commit(
    repo_path: &str,
    message: &str,
    author_name: &str,
    author_email: &str,
) -> GitfastResult<String> {
    let path = repo_path.to_string();
    let msg = message.to_string();
    let name = author_name.to_string();
    let email = author_email.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;
        let mut index = repo
            .index()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let tree_id = index
            .write_tree()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let tree = repo
            .find_tree(tree_id)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        let sig = git2::Signature::now(&name, &email)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        let parent = repo.head().ok().and_then(|r| r.peel_to_commit().ok());
        let oid = if let Some(parent_commit) = parent {
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                &msg,
                &tree,
                &[&parent_commit],
            )
        } else {
            repo.commit(Some("HEAD"), &sig, &sig, &msg, &tree, &[])
        }
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        Ok(oid.to_string())
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Returns status as pretty-printed JSON.
pub async fn get_status_json(repo_path: &str) -> GitfastResult<String> {
    let entries = get_status(repo_path).await?;
    serde_json::to_string_pretty(&entries)
        .map_err(|e| GitfastError::SerializationError(e.to_string()))
}

/// Discard changes to a single tracked file (restore from HEAD).
pub fn discard_file(repo: &Repository, file_path: &str) -> Result<()> {
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.path(file_path);
    checkout.force();
    repo.checkout_head(Some(&mut checkout))
        .map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

/// Discard ALL unstaged changes (restore entire working tree from HEAD).
pub fn discard_all(repo: &Repository) -> Result<()> {
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.force();
    repo.checkout_head(Some(&mut checkout))
        .map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

/// Delete an untracked file (new files not yet in HEAD).
pub fn delete_untracked(repo: &Repository, file_path: &str) -> Result<()> {
    let repo_path = repo
        .workdir()
        .ok_or_else(|| anyhow::anyhow!("No working directory"))?;
    let full_path = repo_path.join(file_path);
    if full_path.exists() {
        std::fs::remove_file(&full_path)
            .map_err(|e| anyhow::anyhow!(e))?;
    }
    Ok(())
}

pub fn cherry_pick(repo_path: &str, commit_hash: &str) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .arg("cherry-pick")
        .arg(commit_hash)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if output.status.success() {
        Ok(format!("Cherry-picked commit {}", &commit_hash[..7]))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

pub fn revert_commit(repo_path: &str, commit_hash: &str) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .arg("revert")
        .arg("--no-edit")
        .arg(commit_hash)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if output.status.success() {
        Ok(format!("Reverted commit {}", &commit_hash[..7]))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

pub fn reset_to_commit(
    repo_path: &str,
    commit_hash: &str,
    mode: &str, // "soft", "mixed", "hard"
) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .arg("reset")
        .arg(format!("--{}", mode))
        .arg(commit_hash)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if output.status.success() {
        Ok(format!("Reset to {} ({})", &commit_hash[..7], mode))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

pub fn copy_commit_hash(hash: &str) -> String {
    hash.to_string()
}
