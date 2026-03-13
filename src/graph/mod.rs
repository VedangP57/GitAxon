//! Graph module for commit history and revision graphs.

mod lanes;

pub use lanes::{assign_lanes, Edge, EdgeType, LanedCommit};

use crate::cache::CommitNode;
use crate::errors::{GitfastError, GitfastResult};
use gix::revision::walk::Sorting;

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

/// Walks the commit graph from HEAD and returns paginated commit nodes.
pub async fn get_commits(
    repo_path: &str,
    limit: usize,
    offset: usize,
) -> GitfastResult<Vec<CommitNode>> {
    let repo = open_repo(repo_path).await?;

    tokio::task::spawn_blocking(move || {
        let head_id = repo
            .head_id()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        let walk = repo
            .rev_walk([head_id])
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

/// Returns the total number of commits reachable from HEAD.
pub async fn get_commit_count(repo_path: &str) -> GitfastResult<usize> {
    let repo = open_repo(repo_path).await?;

    tokio::task::spawn_blocking(move || {
        let head_id = repo
            .head_id()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        let walk = repo
            .rev_walk([head_id])
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
