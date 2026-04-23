//! Tag management module.

use std::path::Path;

use git2::Repository;
use serde::{Deserialize, Serialize};

use crate::errors::{GitfastError, GitfastResult};

/// Information about a git tag.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagInfo {
    pub name: String,
    pub hash: String,
    pub message: String,
    pub is_annotated: bool,
    pub tagger_name: String,
    pub tagger_date: i64,
}

fn open_repo(repo_path: &str) -> Result<Repository, GitfastError> {
    let path = Path::new(repo_path);
    if !path.exists() {
        return Err(GitfastError::RepoNotFound(repo_path.to_string()));
    }
    Repository::open(repo_path).map_err(|e| GitfastError::NotAGitRepo(e.to_string()))
}

/// Returns all tags in the repository.
pub async fn list_tags(repo_path: &str) -> GitfastResult<Vec<TagInfo>> {
    let path = repo_path.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;
        let mut tags = Vec::new();

        let tag_names = repo
            .tag_names(None)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        for name in tag_names.iter().flatten() {
            let refname = format!("refs/tags/{}", name);
            let reference = match repo.find_reference(&refname) {
                Ok(r) => r,
                Err(_) => continue,
            };

            let resolved = match reference.resolve() {
                Ok(r) => r,
                Err(_) => continue,
            };
            let target_oid = match resolved.target() {
                Some(oid) => oid,
                None => continue,
            };

            // Check if this is an annotated tag by looking up the tag object
            let ref_target = match reference.target() {
                Some(oid) => oid,
                None => continue,
            };

            let (message, is_annotated, tagger_name, tagger_date) =
                if let Ok(tag_obj) = repo.find_tag(ref_target) {
                    let msg = tag_obj.message().unwrap_or("").to_string();
                    let (tname, tdate) = tag_obj
                        .tagger()
                        .map(|t| {
                            (
                                t.name().unwrap_or("").to_string(),
                                t.when().seconds(),
                            )
                        })
                        .unwrap_or_default();
                    (msg, true, tname, tdate)
                } else {
                    (String::new(), false, String::new(), 0)
                };

            tags.push(TagInfo {
                name: name.to_string(),
                hash: target_oid.to_string(),
                message,
                is_annotated,
                tagger_name,
                tagger_date,
            });
        }

        // Sort by name
        tags.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(tags)
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Returns all tags as a JSON string.
pub async fn list_tags_json(repo_path: &str) -> GitfastResult<String> {
    let tags = list_tags(repo_path).await?;
    serde_json::to_string(&tags).map_err(|e| GitfastError::SerializationError(e.to_string()))
}

/// Creates a new tag at a given commit.
pub async fn create_tag(
    repo_path: &str,
    name: &str,
    target_hash: &str,
    message: &str,
) -> GitfastResult<()> {
    let path = repo_path.to_string();
    let name = name.to_string();
    let target = target_hash.to_string();
    let msg = message.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;
        let obj = repo
            .revparse_single(&target)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        if msg.is_empty() {
            // Lightweight tag
            repo.tag_lightweight(&name, &obj, false)
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        } else {
            // Annotated tag
            let sig = repo
                .signature()
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            repo.tag(&name, &obj, &sig, &msg, false)
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        }
        Ok(())
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Deletes a local tag.
pub async fn delete_tag(repo_path: &str, name: &str) -> GitfastResult<()> {
    let path = repo_path.to_string();
    let name = name.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;
        let refname = format!("refs/tags/{}", name);
        let mut reference = repo
            .find_reference(&refname)
            .map_err(|e| GitfastError::GitOperationFailed(format!("tag '{}' not found: {}", name, e)))?;
        reference
            .delete()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        Ok(())
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Pushes a tag to a remote.
pub fn push_tag(repo_path: &str, remote_name: &str, tag_name: &str) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .args(["push", remote_name, &format!("refs/tags/{}", tag_name)])
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if output.status.success() {
        Ok(format!("Pushed tag '{}' to {}", tag_name, remote_name))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Deletes a tag from a remote.
pub fn delete_remote_tag(
    repo_path: &str,
    remote_name: &str,
    tag_name: &str,
) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .args([
            "push",
            remote_name,
            &format!(":refs/tags/{}", tag_name),
        ])
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if output.status.success() {
        Ok(format!(
            "Deleted tag '{}' from {}",
            tag_name, remote_name
        ))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}
