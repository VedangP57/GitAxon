//! Diff module for comparing and displaying changes.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{TimeZone, Utc};
use git2::{Delta, DiffFindOptions, DiffOptions, Patch, Repository};
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

/// Blame metadata for a single source line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlameLine {
    pub line_no: usize,
    pub content: String,
    pub commit_hash: String,
    pub short_hash: String,
    pub author: String,
    pub author_email: String,
    pub date: String,
    pub timestamp: i64,
    pub summary: String,
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

/// Build a synthetic all-added DiffFile from raw file bytes (for untracked files).
fn untracked_to_diff_file(path: &str, full_path: &Path) -> Option<DiffFile> {
    let bytes = fs::read(full_path).ok()?;
    // Skip binary files
    if bytes.contains(&0u8) {
        return Some(DiffFile {
            old_path: None,
            new_path: Some(path.to_string()),
            status: FileStatus::Added,
            hunks: vec![],
        });
    }
    let content = String::from_utf8_lossy(&bytes);
    let lines: Vec<DiffLine> = content
        .lines()
        .enumerate()
        .map(|(i, l)| DiffLine {
            content: format!("{}\n", l),
            line_type: LineType::Added,
            old_line_no: None,
            new_line_no: Some((i + 1) as u32),
        })
        .collect();
    let n = lines.len() as u32;
    Some(DiffFile {
        old_path: None,
        new_path: Some(path.to_string()),
        status: FileStatus::Added,
        hunks: if n == 0 {
            vec![]
        } else {
            vec![DiffHunk {
                old_start: 0,
                old_lines: 0,
                new_start: 1,
                new_lines: n,
                lines,
            }]
        },
    })
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
            let repo_root = repo.workdir().map(|p| p.to_path_buf()).unwrap_or_default();
            let mut opts = DiffOptions::new();
            opts.include_untracked(true).recurse_untracked_dirs(true);
            let mut diff = repo
                .diff_index_to_workdir(None, Some(&mut opts))
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

            // process_diff uses Patch::from_diff which returns None for Delta::Untracked,
            // so those deltas are silently dropped. Handle them here first.
            let num_deltas = diff.deltas().count();
            let mut untracked: Vec<DiffFile> = Vec::new();
            for i in 0..num_deltas {
                let delta = diff.get_delta(i).ok_or_else(|| {
                    GitfastError::GitOperationFailed("delta index out of range".into())
                })?;
                if delta.status() == Delta::Untracked {
                    if let Some(rel) = delta.new_file().path().map(|p| p.to_string_lossy().into_owned()) {
                        let full = repo_root.join(&rel);
                        if let Some(df) = untracked_to_diff_file(&rel, &full) {
                            untracked.push(df);
                        }
                    }
                }
            }

            let mut files = process_diff(&mut diff)?;
            files.extend(untracked);
            Ok(files)
        })
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Returns diff of index against HEAD (staged changes).
/// On a new repo with no commits (unborn HEAD), diffs against the empty tree.
pub async fn diff_staged(repo_path: &str) -> GitfastResult<Vec<DiffFile>> {
    let path = repo_path.to_string();
    tokio::task::spawn_blocking(move || {
        run_diff(&path, |repo| {
            let head_tree = match repo.head() {
                Ok(head) => Some(
                    head.peel_to_tree()
                        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?,
                ),
                Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
                Err(e) => return Err(GitfastError::GitOperationFailed(e.to_string())),
            };
            let mut diff = repo
                .diff_tree_to_index(head_tree.as_ref(), None, None)
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
/// Returns empty vec on a new repo with no commits (unborn HEAD).
pub fn get_staged_files_from_repo(repo: &Repository) -> GitfastResult<Vec<DiffFile>> {
    let head_tree = match repo.head() {
        Ok(head) => Some(
            head.peel_to_tree()
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?,
        ),
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
        Err(e) => return Err(GitfastError::GitOperationFailed(e.to_string())),
    };
    let mut diff = repo
        .diff_tree_to_index(head_tree.as_ref(), None, None)
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

fn ensure_utf8_text(bytes: &[u8]) -> Result<String, GitfastError> {
    const SAMPLE: usize = 8000;
    let sample = bytes.len().min(SAMPLE);
    let slice = &bytes[..sample];
    if slice.iter().any(|b| *b == 0) {
        return Err(GitfastError::GitOperationFailed(
            "file appears to be binary".into(),
        ));
    }
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

fn workdir_path_under_repo(repo: &Repository, rel_path: &str) -> GitfastResult<PathBuf> {
    let workdir = repo.workdir().ok_or_else(|| {
        GitfastError::GitOperationFailed("bare repository has no working tree".into())
    })?;
    let joined = workdir.join(rel_path);
    let workdir_canon = workdir.canonicalize().map_err(|e| {
        GitfastError::GitOperationFailed(format!("working directory: {e}"))
    })?;
    let full_canon = joined.canonicalize().map_err(|e| {
        GitfastError::GitOperationFailed(format!("file not readable: {e}"))
    })?;
    if !full_canon.starts_with(&workdir_canon) {
        return Err(GitfastError::GitOperationFailed(
            "path escapes repository root".into(),
        ));
    }
    Ok(full_canon)
}

fn read_working_tree_utf8(repo: &Repository, rel_path: &str) -> GitfastResult<String> {
    let path = workdir_path_under_repo(repo, rel_path)?;
    let bytes = fs::read(&path).map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
    ensure_utf8_text(&bytes)
}

fn read_index_utf8(repo: &Repository, rel_path: &str) -> GitfastResult<String> {
    let index = repo
        .index()
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
    let entry = index
        .get_path(Path::new(rel_path), 0)
        .ok_or_else(|| GitfastError::GitOperationFailed("file is not staged".into()))?;
    let blob = repo
        .find_blob(entry.id)
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
    ensure_utf8_text(blob.content())
}

fn read_commit_tree_utf8(
    repo: &Repository,
    commit_hash: &str,
    rel_path: &str,
) -> GitfastResult<String> {
    let obj = repo
        .revparse_single(commit_hash)
        .map_err(|_| GitfastError::CommitNotFound(commit_hash.to_string()))?;
    let commit = obj
        .peel_to_commit()
        .map_err(|_| GitfastError::CommitNotFound(commit_hash.to_string()))?;
    let tree = commit
        .tree()
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
    let entry = tree
        .get_path(Path::new(rel_path))
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
    let blob = repo
        .find_blob(entry.id())
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
    ensure_utf8_text(blob.content())
}

/// Parent commit's tree (for a deleted path in a commit diff).
fn read_commit_parent_tree_utf8(
    repo: &Repository,
    commit_hash: &str,
    rel_path: &str,
) -> GitfastResult<String> {
    let obj = repo
        .revparse_single(commit_hash)
        .map_err(|_| GitfastError::CommitNotFound(commit_hash.to_string()))?;
    let commit = obj
        .peel_to_commit()
        .map_err(|_| GitfastError::CommitNotFound(commit_hash.to_string()))?;
    let parent = commit.parent(0).map_err(|_| {
        GitfastError::GitOperationFailed(
            "cannot load file before deletion: commit has no parent".into(),
        )
    })?;
    let tree = parent
        .tree()
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
    let entry = tree
        .get_path(Path::new(rel_path))
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
    let blob = repo
        .find_blob(entry.id())
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
    ensure_utf8_text(blob.content())
}

/// Full file contents for the diff viewer "file" tab (`working-tree`, `staged`, or `commit`).
///
/// `file_status` is the diff file status string (e.g. `"Deleted"`); when viewing a commit and the
/// file was deleted in that commit, content is read from the first parent tree.
pub fn read_diff_file_content(
    repo_path: &str,
    file_path: &str,
    mode: &str,
    commit_hash: Option<&str>,
    file_status: Option<&str>,
) -> GitfastResult<String> {
    let path = Path::new(repo_path);
    if !path.exists() {
        return Err(GitfastError::RepoNotFound(repo_path.to_string()));
    }
    let repo = Repository::open(repo_path)
        .map_err(|e| GitfastError::NotAGitRepo(e.to_string()))?;

    match mode {
        "working-tree" => read_working_tree_utf8(&repo, file_path),
        "staged" => read_index_utf8(&repo, file_path),
        "commit" => {
            let hash = commit_hash.ok_or_else(|| {
                GitfastError::GitOperationFailed("commit hash required for commit mode".into())
            })?;
            if file_status == Some("Deleted") {
                read_commit_parent_tree_utf8(&repo, hash, file_path)
            } else {
                read_commit_tree_utf8(&repo, hash, file_path)
            }
        }
        _ => Err(GitfastError::GitOperationFailed(format!(
            "unknown diff mode: {mode}"
        ))),
    }
}

/// Returns git blame metadata for each line in a file.
pub fn git_blame(
    repo_path: &str,
    file_path: &str,
    commit_hash: Option<&str>,
) -> Result<Vec<BlameLine>, String> {
    let mut args = vec![
        "blame".to_string(),
        "--porcelain".to_string(),
        "--".to_string(),
        file_path.to_string(),
    ];

    if let Some(hash) = commit_hash {
        args.insert(2, hash.to_string());
    }

    let output = Command::new("git")
        .current_dir(repo_path)
        .args(&args)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_blame_porcelain(&stdout)
}

fn parse_blame_porcelain(output: &str) -> Result<Vec<BlameLine>, String> {
    let mut lines = Vec::new();
    let mut current_hash = String::new();
    let mut current_author = String::new();
    let mut current_email = String::new();
    let mut current_timestamp: i64 = 0;
    let mut current_summary = String::new();
    let mut current_line_no: usize = 0;

    for line in output.lines() {
        if let Some(content) = line.strip_prefix('\t') {
            let date = Utc
                .timestamp_opt(current_timestamp, 0)
                .single()
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "1970-01-01".to_string());

            let short_hash = if current_hash.len() >= 7 {
                current_hash[..7].to_string()
            } else {
                current_hash.clone()
            };

            lines.push(BlameLine {
                line_no: current_line_no,
                content: content.to_string(),
                commit_hash: current_hash.clone(),
                short_hash,
                author: current_author.clone(),
                author_email: current_email.clone(),
                date,
                timestamp: current_timestamp,
                summary: current_summary.clone(),
            });
            continue;
        }

        if let Some((hash, rest)) = line.split_once(' ') {
            if hash.len() == 40 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
                let mut parts = rest.split_whitespace();
                let _orig_no = parts.next();
                let final_no = parts.next();
                current_hash = hash.to_string();
                current_line_no = final_no.and_then(|n| n.parse().ok()).unwrap_or(0);
                continue;
            }
        }

        if let Some(rest) = line.strip_prefix("author ") {
            current_author = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("author-mail ") {
            current_email = rest.trim_matches(&['<', '>'][..]).to_string();
        } else if let Some(rest) = line.strip_prefix("author-time ") {
            current_timestamp = rest.parse().unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("summary ") {
            current_summary = rest.to_string();
        }
    }

    Ok(lines)
}

/// Parse a unified diff (e.g. from `git stash show -p` or `git diff`) into DiffFile objects.
pub fn parse_unified_diff(patch: &str) -> Vec<DiffFile> {
    let mut files: Vec<DiffFile> = Vec::new();
    let mut current_file: Option<DiffFile> = None;
    let mut current_hunk: Option<DiffHunk> = None;
    let mut old_line: u32 = 0;
    let mut new_line: u32 = 0;

    for raw_line in patch.lines() {
        if raw_line.starts_with("diff --git ") {
            // Flush previous hunk/file
            if let Some(ref mut f) = current_file {
                if let Some(h) = current_hunk.take() {
                    f.hunks.push(h);
                }
                files.push(f.clone());
            }
            current_file = Some(DiffFile {
                old_path: None,
                new_path: None,
                status: FileStatus::Modified,
                hunks: Vec::new(),
            });
            current_hunk = None;
        } else if raw_line.starts_with("--- ") {
            if let Some(ref mut f) = current_file {
                let path = raw_line.strip_prefix("--- a/").unwrap_or(
                    raw_line.strip_prefix("--- ").unwrap_or(""),
                );
                if path != "/dev/null" {
                    f.old_path = Some(path.to_string());
                }
            }
        } else if raw_line.starts_with("+++ ") {
            if let Some(ref mut f) = current_file {
                let path = raw_line.strip_prefix("+++ b/").unwrap_or(
                    raw_line.strip_prefix("+++ ").unwrap_or(""),
                );
                if path == "/dev/null" {
                    f.status = FileStatus::Deleted;
                } else {
                    f.new_path = Some(path.to_string());
                    if f.old_path.is_none() {
                        f.status = FileStatus::Added;
                    }
                }
            }
        } else if raw_line.starts_with("@@ ") {
            // Flush previous hunk
            if let Some(ref mut f) = current_file {
                if let Some(h) = current_hunk.take() {
                    f.hunks.push(h);
                }
            }
            // Parse @@ -old_start,old_lines +new_start,new_lines @@
            let parts: Vec<&str> = raw_line.split_whitespace().collect();
            let (os, ol) = parse_hunk_range(parts.get(1).unwrap_or(&"-0,0"));
            let (ns, nl) = parse_hunk_range(parts.get(2).unwrap_or(&"+0,0"));
            old_line = os;
            new_line = ns;
            current_hunk = Some(DiffHunk {
                old_start: os,
                old_lines: ol,
                new_start: ns,
                new_lines: nl,
                lines: Vec::new(),
            });
        } else if let Some(ref mut hunk) = current_hunk {
            if raw_line.starts_with('+') {
                hunk.lines.push(DiffLine {
                    content: raw_line[1..].to_string(),
                    line_type: LineType::Added,
                    old_line_no: None,
                    new_line_no: Some(new_line),
                });
                new_line += 1;
            } else if raw_line.starts_with('-') {
                hunk.lines.push(DiffLine {
                    content: raw_line[1..].to_string(),
                    line_type: LineType::Deleted,
                    old_line_no: Some(old_line),
                    new_line_no: None,
                });
                old_line += 1;
            } else if raw_line.starts_with(' ') || raw_line.is_empty() {
                let content = if raw_line.is_empty() { "" } else { &raw_line[1..] };
                hunk.lines.push(DiffLine {
                    content: content.to_string(),
                    line_type: LineType::Context,
                    old_line_no: Some(old_line),
                    new_line_no: Some(new_line),
                });
                old_line += 1;
                new_line += 1;
            }
        }
        // Check for rename detection
        if raw_line.starts_with("rename from ") || raw_line.starts_with("similarity index") {
            if let Some(ref mut f) = current_file {
                f.status = FileStatus::Renamed;
            }
        }
    }

    // Flush final file
    if let Some(ref mut f) = current_file {
        if let Some(h) = current_hunk.take() {
            f.hunks.push(h);
        }
        files.push(f.clone());
    }

    files
}

fn parse_hunk_range(s: &str) -> (u32, u32) {
    let s = s.trim_start_matches(['-', '+'].as_ref());
    let parts: Vec<&str> = s.split(',').collect();
    let start = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
    let lines = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(1);
    (start, lines)
}
