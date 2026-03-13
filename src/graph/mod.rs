//! Graph module for commit history and revision graphs.

mod lanes;

pub use lanes::{assign_lanes, Edge, EdgeType, LanedCommit};

use crate::cache::CommitNode;
use crate::errors::{GitfastError, GitfastResult};
use gix::revision::walk::Sorting;
use std::collections::HashSet;

/// Opens a gix repository at the given path.
pub async fn open_repo(path: &str) -> GitfastResult<gix::Repository> {
    let path_exists = tokio::fs::metadata(path).await.is_ok();
    if !path_exists {
        return Err(GitfastError::RepoNotFound(path.to_string()));
    }

    let path = path.to_string();
    tokio::task::spawn_blocking(move || {
        gix::open(&path).map_err(|e| GitfastError::NotAGitRepo(format!("{}", e)))
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Collects all ref tips (branches, remotes, tags) as gix ObjectIds for walking the full graph.
/// Uses git2 for reliable ref iteration (branches + remotes + tags), then gix for the rev walk.
fn collect_all_ref_tips(repo_path: &str) -> GitfastResult<Vec<gix::ObjectId>> {
    let git2_repo = git2::Repository::open(repo_path)
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

    let mut tip_ids: HashSet<gix::ObjectId> = HashSet::new();

    // Local branches
    for branch in git2_repo
        .branches(Some(git2::BranchType::Local))
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
    {
        let (branch, _) = branch.map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        if let Ok(commit) = branch.get().peel_to_commit() {
            let hex = commit.id().to_string();
            tip_ids.insert(
                gix::ObjectId::from_hex(hex.as_bytes())
                    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?,
            );
        }
    }

    // Remote branches
    for branch in git2_repo
        .branches(Some(git2::BranchType::Remote))
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
    {
        let (branch, _) = branch.map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        if let Ok(commit) = branch.get().peel_to_commit() {
            let hex = commit.id().to_string();
            tip_ids.insert(
                gix::ObjectId::from_hex(hex.as_bytes())
                    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?,
            );
        }
    }

    // Tags (peel to commit)
    git2_repo
        .tag_foreach(|oid, _| {
            if let Ok(obj) = git2_repo.find_object(oid, None) {
                if let Ok(commit) = obj.peel_to_commit() {
                    let hex = commit.id().to_string();
                    if let Ok(gix_oid) = gix::ObjectId::from_hex(hex.as_bytes()) {
                        tip_ids.insert(gix_oid);
                    }
                }
            }
            true
        })
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

    // Fallback: if no refs, use HEAD (e.g. fresh init)
    if tip_ids.is_empty() {
        if let Ok(head) = git2_repo.head() {
            if let Ok(commit) = head.peel_to_commit() {
                let hex = commit.id().to_string();
                tip_ids.insert(
                    gix::ObjectId::from_hex(hex.as_bytes())
                        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?,
                );
            }
        }
    }

    Ok(tip_ids.into_iter().collect())
}

/// Walks the commit graph from ALL branch/remote/tag tips and returns paginated commit nodes.
/// GitKraken-style: shows the complete repository history in one unified view.
pub async fn get_commits(
    repo_path: &str,
    limit: usize,
    offset: usize,
) -> GitfastResult<Vec<CommitNode>> {
    let repo = open_repo(repo_path).await?;
    let repo_path_owned = repo_path.to_string();

    tokio::task::spawn_blocking(move || {
        let tip_ids = collect_all_ref_tips(&repo_path_owned)?;

        let walk = repo
            .rev_walk(tip_ids)
            .sorting(Sorting::ByCommitTime(Default::default()))
            .all()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        let mut commits = Vec::new();
        for (idx, result) in walk.enumerate() {
            let info = result.map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            let commit = info.object().map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

            let hash = commit.id().to_string();
            let short_hash = hash.chars().take(7).collect::<String>();
            let message = commit
                .message_raw_sloppy()
                .to_string();
            let author = commit
                .author()
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            let author_name = author.name.to_string();
            let author_email = author.email.to_string();
            let timestamp = commit
                .time()
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
                .seconds as i64;
            let parent_hashes: Vec<String> = commit.parent_ids().map(|id| id.to_string()).collect();

            let node = CommitNode {
                hash,
                short_hash,
                message,
                author_name,
                author_email,
                timestamp,
                parent_hashes,
            };

            if idx >= offset {
                commits.push(node);
                if commits.len() >= limit {
                    break;
                }
            }
        }
        Ok::<_, GitfastError>(commits)
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Returns the total number of commits reachable from all refs (branches, remotes, tags).
pub async fn get_commit_count(repo_path: &str) -> GitfastResult<usize> {
    let repo = open_repo(repo_path).await?;
    let repo_path_owned = repo_path.to_string();

    tokio::task::spawn_blocking(move || {
        let tip_ids = collect_all_ref_tips(&repo_path_owned)?;

        let walk = repo
            .rev_walk(tip_ids)
            .all()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        let count = walk
            .filter_map(|r| r.ok())
            .count();
        Ok::<_, GitfastError>(count)
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Returns commits as pretty-printed JSON.
pub async fn get_commits_json(
    repo_path: &str,
    limit: usize,
    offset: usize,
) -> GitfastResult<String> {
    let commits = get_commits(repo_path, limit, offset).await?;
    serde_json::to_string_pretty(&commits)
        .map_err(|e| GitfastError::SerializationError(e.to_string()))
}

/// Returns commits with lane assignments as pretty-printed JSON.
pub async fn get_laned_commits_json(
    repo_path: &str,
    limit: usize,
    offset: usize,
) -> GitfastResult<String> {
    let commits = get_commits(repo_path, limit, offset).await?;
    let laned = lanes::assign_lanes(commits);
    serde_json::to_string_pretty(&laned)
        .map_err(|e| GitfastError::SerializationError(e.to_string()))
}
