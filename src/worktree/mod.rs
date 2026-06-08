//! Git worktree management.

use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeInfo {
    pub path: String,
    pub branch: Option<String>,
    pub head_hash: String,
    pub is_main: bool,
    pub is_detached: bool,
}

/// List all worktrees for the given repository.
pub fn list_worktrees(repo_path: &str) -> Result<Vec<WorktreeInfo>, String> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["worktree", "list", "--porcelain"])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut worktrees = Vec::new();
    let mut current = WorktreeInfo {
        path: String::new(),
        branch: None,
        head_hash: String::new(),
        is_main: false,
        is_detached: false,
    };
    let mut is_first = true;

    for line in stdout.lines() {
        if line.is_empty() {
            if !current.path.is_empty() {
                current.is_main = is_first;
                is_first = false;
                worktrees.push(current.clone());
                current = WorktreeInfo {
                    path: String::new(),
                    branch: None,
                    head_hash: String::new(),
                    is_main: false,
                    is_detached: false,
                };
            }
            continue;
        }

        if let Some(path) = line.strip_prefix("worktree ") {
            current.path = path.to_string();
        } else if let Some(hash) = line.strip_prefix("HEAD ") {
            current.head_hash = hash.to_string();
        } else if let Some(branch) = line.strip_prefix("branch ") {
            // branch refs/heads/main -> main
            current.branch = Some(
                branch
                    .strip_prefix("refs/heads/")
                    .unwrap_or(branch)
                    .to_string(),
            );
        } else if line == "detached" {
            current.is_detached = true;
        }
    }

    // Flush last entry
    if !current.path.is_empty() {
        current.is_main = is_first;
        worktrees.push(current);
    }

    Ok(worktrees)
}

/// Add a new worktree at the given path for the given branch.
pub fn add_worktree(repo_path: &str, path: &str, branch: &str) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["worktree", "add", path, branch])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(format!("Created worktree at {} on branch {}", path, branch))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Remove a worktree. Use force=true to remove even if it has modifications.
pub fn remove_worktree(repo_path: &str, path: &str, force: bool) -> Result<String, String> {
    let mut args = vec!["worktree", "remove", path];
    if force {
        args.push("--force");
    }

    let output = Command::new("git")
        .current_dir(repo_path)
        .args(&args)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(format!("Removed worktree at {}", path))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}
