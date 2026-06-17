//! Branches module for branch management.

use git2::{build::CheckoutBuilder, BranchType, MergeOptions, Repository, StatusOptions};
use serde::{Deserialize, Serialize};

use crate::errors::{GitfastError, GitfastResult};

/// Information about a git branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub name: String,
    pub is_remote: bool,
    pub is_head: bool,
    pub upstream: Option<String>,
    pub tip_hash: String,
    pub tip_message: String,
}

fn open_repo(repo_path: &str) -> Result<Repository, GitfastError> {
    crate::repo_pool::open_repo(repo_path)
}

fn branch_to_info(_repo: &Repository, branch: &git2::Branch, is_remote: bool) -> GitfastResult<BranchInfo> {
    let name = branch
        .name()
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
        .map(String::from)
        .unwrap_or_else(|| String::from("<invalid-utf8>"));
    let is_head = !is_remote && branch.is_head();

    let upstream = if is_remote {
        None
    } else {
        branch
            .upstream()
            .ok()
            .and_then(|u| u.name().ok().flatten().map(String::from))
    };

    let reference = branch.get();
    let commit = reference
        .peel_to_commit()
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
    let tip_hash = commit.id().to_string();
    let tip_message = commit
        .message()
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("")
        .to_string();

    Ok(BranchInfo {
        name,
        is_remote,
        is_head,
        upstream,
        tip_hash,
        tip_message,
    })
}

/// Returns all local branches and remote tracking branches.
/// The current HEAD branch has `is_head: true`.
pub async fn list_branches(repo_path: &str) -> GitfastResult<Vec<BranchInfo>> {
    let path = repo_path.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;
        let mut branches = Vec::new();

        for branch_type in [BranchType::Local, BranchType::Remote] {
            let branch_iter = repo
                .branches(Some(branch_type))
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            for result in branch_iter {
                let (branch, _) = result.map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
                let is_remote = branch_type == BranchType::Remote;
                let info = branch_to_info(&repo, &branch, is_remote)?;
                branches.push(info);
            }
        }

        Ok(branches)
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Creates a new branch from `from_ref` (branch name, tag, or commit hash).
pub async fn create_branch(repo_path: &str, name: &str, from_ref: &str) -> GitfastResult<()> {
    let path = repo_path.to_string();
    let name = name.to_string();
    let from_ref = from_ref.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;
        let commit = repo
            .revparse_single(&from_ref)
            .and_then(|obj| obj.peel_to_commit())
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        repo.branch(&name, &commit, false)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        Ok(())
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Deletes a local branch. If `force` is false, refuses when unmerged.
pub async fn delete_branch(repo_path: &str, name: &str, force: bool) -> GitfastResult<()> {
    let path = repo_path.to_string();
    let name = name.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;
        let refname = format!("refs/heads/{}", name);

        // Guard: never delete the currently checked-out branch
        let head_name = repo
            .head()
            .ok()
            .and_then(|h| h.name().map(String::from))
            .unwrap_or_default();
        if head_name == refname {
            return Err(GitfastError::GitOperationFailed(
                format!("Cannot delete '{}': it is the currently checked-out branch.", name),
            ));
        }

        if force {
            let mut reference = repo
                .find_reference(&refname)
                .map_err(|_| GitfastError::BranchNotFound(name.clone()))?;
            reference.delete().map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        } else {
            let mut branch = repo
                .find_branch(&name, BranchType::Local)
                .map_err(|_| GitfastError::BranchNotFound(name.clone()))?;
            branch.delete().map_err(|e| {
                GitfastError::GitOperationFailed(format!(
                    "cannot delete branch '{}': {}. Use force=true to delete unmerged branches",
                    name, e
                ))
            })?;
        }
        Ok(())
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Renames a local branch.
pub async fn rename_branch(repo_path: &str, old_name: &str, new_name: &str) -> GitfastResult<()> {
    let path = repo_path.to_string();
    let old_name = old_name.to_string();
    let new_name = new_name.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;
        let mut branch = repo
            .find_branch(&old_name, BranchType::Local)
            .map_err(|_| GitfastError::BranchNotFound(old_name))?;
        branch
            .rename(&new_name, false)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        Ok(())
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Checks out a branch. Fails if the working tree has uncommitted changes.
pub async fn checkout_branch(repo_path: &str, name: &str) -> GitfastResult<()> {
    let path = repo_path.to_string();
    let name = name.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;

        // Check for uncommitted changes
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .include_ignored(false)
            .renames_from_rewrites(false);
        let statuses = repo
            .statuses(Some(&mut opts))
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let has_changes = statuses.iter().any(|e| {
            let s = e.status();
            !s.is_ignored()
                && (s.is_wt_new()
                    || s.is_wt_modified()
                    || s.is_wt_deleted()
                    || s.is_wt_typechange()
                    || s.is_wt_renamed()
                    || s.is_index_new()
                    || s.is_index_modified()
                    || s.is_index_deleted()
                    || s.is_index_typechange()
                    || s.is_index_renamed()
                    || s.is_conflicted())
        });
        if has_changes {
            return Err(GitfastError::GitOperationFailed(
                "cannot checkout: working tree has uncommitted changes. Commit or stash them first."
                    .to_string(),
            ));
        }

        let (object, reference) = repo
            .revparse_ext(&name)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        let target_ref = reference.as_ref().and_then(|r| r.name()).map(String::from);

        repo.checkout_tree(&object, Some(&mut CheckoutBuilder::new()))
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        if let Some(r) = target_ref {
            if r.starts_with("refs/heads/") {
                repo.set_head(&r)
                    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            } else {
                repo.set_head_detached(object.id())
                    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            }
        } else {
            repo.set_head_detached(object.id())
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        }

        Ok(())
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Merges a branch into the current HEAD branch.
pub async fn merge_branch(repo_path: &str, branch_name: &str) -> GitfastResult<()> {
    let path = repo_path.to_string();
    let branch_name = branch_name.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;

        // C3: Guard against detached HEAD — merge would create an orphaned commit
        let head = repo.head().map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        if !head.is_branch() {
            return Err(GitfastError::GitOperationFailed(
                "Cannot merge: HEAD is detached. Checkout a branch first.".to_string(),
            ));
        }

        let branch_oid = repo
            .refname_to_id(&format!("refs/heads/{}", branch_name))
            .map_err(|_| GitfastError::BranchNotFound(branch_name.clone()))?;
        let branch_commit = repo
            .find_commit(branch_oid)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let annotated = repo
            .find_annotated_commit(branch_oid)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        let (analysis, _) = repo
            .merge_analysis(&[&annotated])
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        if analysis.is_fast_forward() {
            // C2: resolve symbolic HEAD → concrete branch ref before calling set_target
            let mut branch_ref = head
                .resolve()
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            branch_ref
                .set_target(branch_oid, "merge: Fast-forward")
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            repo.checkout_head(Some(&mut CheckoutBuilder::new()))
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            return Ok(());
        }

        let mut merge_opts = MergeOptions::new();
        let mut checkout = CheckoutBuilder::new();
        repo.merge(&[&annotated], Some(&mut merge_opts), Some(&mut checkout))
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        let mut index = repo.index().map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        if index.has_conflicts() {
            return Err(GitfastError::GitOperationFailed(
                "merge resulted in conflicts".to_string(),
            ));
        }

        let head_commit = repo
            .head()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
            .peel_to_commit()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let sig = repo.signature().map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let tree_oid = index.write_tree().map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let tree = repo.find_tree(tree_oid).map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let message = format!("Merge branch '{}'", branch_name);
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &message,
            &tree,
            &[&head_commit, &branch_commit],
        )
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        Ok(())
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Returns all branches as a JSON string.
pub async fn list_branches_json(repo_path: &str) -> GitfastResult<String> {
    let branches = list_branches(repo_path).await?;
    serde_json::to_string_pretty(&branches)
        .map_err(|e| GitfastError::SerializationError(e.to_string()))
}
