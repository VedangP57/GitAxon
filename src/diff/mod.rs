//! Diff module for comparing and displaying changes.

use std::path::Path;

use git2::{Delta, DiffFindOptions, Patch, Repository};
use serde::{Deserialize, Serialize};

use crate::errors::{GitfastError, GitfastResult};

/// File status in a diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum FileStatus {
    Added,
    Deleted,
    Modified,
    Renamed,
    Copied,
}

/// Line type in a diff hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum LineType {
    Added,
    Deleted,
    Context,
}

/// A single line in a diff hunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    /// Line content (including newline).
    pub content: String,
    /// Type of line change.
    pub line_type: LineType,
    /// Line number in old file (None for added lines).
    pub old_line_no: Option<u32>,
    /// Line number in new file (None for deleted lines).
    pub new_line_no: Option<u32>,
}

/// A hunk of changed lines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    /// Starting line in old file.
    pub old_start: u32,
    /// Number of lines in old file.
    pub old_lines: u32,
    /// Starting line in new file.
    pub new_start: u32,
    /// Number of lines in new file.
    pub new_lines: u32,
    /// Lines in this hunk.
    pub lines: Vec<DiffLine>,
}

/// A file changed in a diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffFile {
    /// Path in old version (None for added files).
    pub old_path: Option<String>,
    /// Path in new version (None for deleted files).
    pub new_path: Option<String>,
    /// File status.
    pub status: FileStatus,
    /// Hunks of changes.
    pub hunks: Vec<DiffHunk>,
}

fn delta_to_file_status(delta: Delta) -> Option<FileStatus> {
    match delta {
        Delta::Added => Some(FileStatus::Added),
        Delta::Deleted => Some(FileStatus::Deleted),
        Delta::Modified => Some(FileStatus::Modified),
        Delta::Renamed => Some(FileStatus::Renamed),
        Delta::Copied => Some(FileStatus::Copied),
        Delta::Typechange => Some(FileStatus::Modified),
        Delta::Untracked => Some(FileStatus::Added),
        _ => None,
    }
}

fn process_diff(diff: &mut git2::Diff) -> Result<Vec<DiffFile>, GitfastError> {
    let mut find_opts = DiffFindOptions::new();
    find_opts.renames(true).copies(true);
    diff.find_similar(Some(&mut find_opts))
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

    let mut files = Vec::new();
    let num_deltas = diff.deltas().count();

    for i in 0..num_deltas {
        let patch = Patch::from_diff(diff, i)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        if let Some(patch) = patch {
            let delta = patch.delta();
            if let Some(status) = delta_to_file_status(delta.status()) {
                let old_path = delta
                    .old_file()
                    .path()
                    .map(|p| p.to_string_lossy().into_owned());
                let new_path = delta
                    .new_file()
                    .path()
                    .map(|p| p.to_string_lossy().into_owned());

                let mut hunks = Vec::new();
                for h in 0..patch.num_hunks() {
                    let (hunk, _) = patch
                        .hunk(h)
                        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

                    let num_lines = patch
                        .num_lines_in_hunk(h)
                        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

                    let mut lines = Vec::with_capacity(num_lines);
                    for l in 0..num_lines {
                        let line = patch
                            .line_in_hunk(h, l)
                            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

                        let (line_type, old_no, new_no) = match line.origin() {
                            '+' | '>' => (LineType::Added, None, line.new_lineno()),
                            '-' | '<' => (LineType::Deleted, line.old_lineno(), None),
                            _ => (
                                LineType::Context,
                                line.old_lineno(),
                                line.new_lineno(),
                            ),
                        };

                        let content = String::from_utf8_lossy(line.content()).into_owned();
                        lines.push(DiffLine {
                            content,
                            line_type,
                            old_line_no: old_no,
                            new_line_no: new_no,
                        });
                    }

                    hunks.push(DiffHunk {
                        old_start: hunk.old_start(),
                        old_lines: hunk.old_lines(),
                        new_start: hunk.new_start(),
                        new_lines: hunk.new_lines(),
                        lines,
                    });
                }

                files.push(DiffFile {
                    old_path,
                    new_path,
                    status,
                    hunks,
                });
            }
        }
    }

    Ok(files)
}

fn run_diff(
    repo_path: &str,
    f: impl FnOnce(&Repository) -> Result<Vec<DiffFile>, GitfastError>,
) -> GitfastResult<Vec<DiffFile>> {
    let path = Path::new(repo_path);
    if !path.exists() {
        return Err(GitfastError::RepoNotFound(repo_path.to_string()));
    }
    let repo = Repository::open(repo_path)
        .map_err(|e| GitfastError::NotAGitRepo(e.to_string()))?;
    f(&repo)
}

/// Returns diff of a commit against its first parent.
pub async fn diff_commit(
    repo_path: &str,
    commit_hash: &str,
) -> GitfastResult<Vec<DiffFile>> {
    let path = repo_path.to_string();
    let hash = commit_hash.to_string();
    tokio::task::spawn_blocking(move || {
        run_diff(&path, |repo| {
            let obj = repo
                .revparse_single(&hash)
                .map_err(|_| GitfastError::CommitNotFound(hash.clone()))?;
            let commit = obj
                .peel_to_commit()
                .map_err(|_| GitfastError::CommitNotFound(hash.clone()))?;
            let commit_tree = commit
                .tree()
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

            let old_tree = match commit.parent(0) {
                Ok(parent) => Some(
                    parent
                        .tree()
                        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?,
                ),
                Err(_) => None, // Root commit: diff against empty tree
            };

            let mut diff = repo
                .diff_tree_to_tree(old_tree.as_ref(), Some(&commit_tree), None)
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            process_diff(&mut diff)
        })
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Returns diff of working tree against HEAD (unstaged changes).
pub async fn diff_working_tree(repo_path: &str) -> GitfastResult<Vec<DiffFile>> {
    let path = repo_path.to_string();
    tokio::task::spawn_blocking(move || {
        run_diff(&path, |repo| {
            let mut diff = repo
                .diff_index_to_workdir(None, None)
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            process_diff(&mut diff)
        })
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Returns diff of index against HEAD (staged changes).
pub async fn diff_staged(repo_path: &str) -> GitfastResult<Vec<DiffFile>> {
    let path = repo_path.to_string();
    tokio::task::spawn_blocking(move || {
        run_diff(&path, |repo| {
            let head = repo
                .head()
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            let head_tree = head
                .peel_to_tree()
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            let mut diff = repo
                .diff_tree_to_index(Some(&head_tree), None, None)
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            process_diff(&mut diff)
        })
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Returns diff of a commit as pretty-printed JSON.
pub async fn diff_commit_json(
    repo_path: &str,
    commit_hash: &str,
) -> GitfastResult<String> {
    let files = diff_commit(repo_path, commit_hash).await?;
    serde_json::to_string_pretty(&files)
        .map_err(|e| GitfastError::SerializationError(e.to_string()))
}

/// Returns working tree diff as pretty-printed JSON.
pub async fn diff_working_tree_json(repo_path: &str) -> GitfastResult<String> {
    let files = diff_working_tree(repo_path).await?;
    serde_json::to_string_pretty(&files)
        .map_err(|e| GitfastError::SerializationError(e.to_string()))
}

/// Returns staged files for a repository (sync, for use by other modules).
pub fn get_staged_files_from_repo(repo: &Repository) -> GitfastResult<Vec<DiffFile>> {
    let head = repo
        .head()
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
    let head_tree = head
        .peel_to_tree()
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
    let mut diff = repo
        .diff_tree_to_index(Some(&head_tree), None, None)
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
    process_diff(&mut diff)
}

/// Returns unstaged files for a repository (sync, for use by other modules).
pub fn get_unstaged_files_from_repo(repo: &Repository) -> GitfastResult<Vec<DiffFile>> {
    let mut diff = repo
        .diff_index_to_workdir(None, None)
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
    process_diff(&mut diff)
}

/// Returns staged diff as pretty-printed JSON.
pub async fn diff_staged_json(repo_path: &str) -> GitfastResult<String> {
    let files = diff_staged(repo_path).await?;
    serde_json::to_string_pretty(&files)
        .map_err(|e| GitfastError::SerializationError(e.to_string()))
}
