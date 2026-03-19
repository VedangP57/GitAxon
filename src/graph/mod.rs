//! Graph module for commit history and revision graphs.

mod lanes;

pub use lanes::{assign_lanes, Edge, EdgeType, LanedCommit};

use crate::cache::CommitNode;
use crate::errors::{GitfastError, GitfastResult};
use gix::revision::walk::Sorting;
use std::collections::{HashMap, HashSet};

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

/// Temporal topological sort: newest commits at top, topology always valid.
/// DFS from newest commits first, assign index after all descendants are assigned.
/// Result: newest-first order suitable for GitKraken-style graph display.
fn temporal_topological_sort(commits: Vec<CommitNode>) -> Vec<CommitNode> {
    if commits.is_empty() {
        return commits;
    }

    // Build children map (each commit -> list of child hashes)
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    for c in &commits {
        for parent in &c.parent_hashes {
            children
                .entry(parent.clone())
                .or_default()
                .push(c.hash.clone());
        }
    }

    let commit_map: HashMap<String, CommitNode> = commits
        .into_iter()
        .map(|c| (c.hash.clone(), c))
        .collect();

    let mut result: Vec<CommitNode> = Vec::new();
    let mut explored: HashSet<String> = HashSet::new();

    // Sort all commits by timestamp newest first
    // This ensures DFS visits newest children first
    let mut all_commits: Vec<&CommitNode> = commit_map.values().collect();
    all_commits.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    fn dfs(
        hash: &str,
        commit_map: &HashMap<String, CommitNode>,
        children: &HashMap<String, Vec<String>>,
        explored: &mut HashSet<String>,
        result: &mut Vec<CommitNode>,
    ) {
        if explored.contains(hash) {
            return;
        }
        explored.insert(hash.to_string());

        // Visit children (commits that come before this in history)
        // sorted by timestamp newest first
        if let Some(child_hashes) = children.get(hash) {
            let mut sorted_children = child_hashes.clone();
            sorted_children.sort_by(|a, b| {
                let ta = commit_map.get(a).map(|c| c.timestamp).unwrap_or(0);
                let tb = commit_map.get(b).map(|c| c.timestamp).unwrap_or(0);
                tb.cmp(&ta) // newest first
            });
            for child in sorted_children {
                dfs(&child, commit_map, children, explored, result);
            }
        }

        // Add this commit AFTER all descendants
        if let Some(commit) = commit_map.get(hash) {
            result.push(commit.clone());
        }
    }

    // Run DFS from each unvisited commit, newest first
    for commit in &all_commits {
        dfs(
            &commit.hash,
            &commit_map,
            &children,
            &mut explored,
            &mut result,
        );
    }

    // DFS adds each commit after its descendants, so result is already newest-first
    result
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
        let mut sorted = temporal_topological_sort(commits);
        // Ensure newest-first for lane algorithm (children must be processed before parents)
        if !sorted.is_empty() && sorted.first().unwrap().parent_hashes.is_empty() {
            sorted.reverse();
        }
        Ok::<_, GitfastError>(sorted)
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
