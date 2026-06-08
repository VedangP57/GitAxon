//! Conflict detection and resolution for merge/rebase/cherry-pick operations.

use std::fs;
use std::path::Path;
use std::process::Command;

use git2::{Status, StatusOptions};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoOperationState {
    pub in_merge: bool,
    pub in_rebase: bool,
    pub in_cherry_pick: bool,
    pub in_revert: bool,
    pub conflicted_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictFile {
    pub path: String,
    pub ours: String,
    pub theirs: String,
    pub base: String,
    pub merged: String,
}

/// Detect if a merge/rebase/cherry-pick/revert is in progress.
pub fn detect_operation_state(repo_path: &str) -> Result<RepoOperationState, String> {
    let git_dir = Path::new(repo_path).join(".git");

    let in_merge = git_dir.join("MERGE_HEAD").exists();
    let in_rebase = git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists();
    let in_cherry_pick = git_dir.join("CHERRY_PICK_HEAD").exists();
    let in_revert = git_dir.join("REVERT_HEAD").exists();

    // Find conflicted files
    let repo = crate::repo_pool::open_repo(repo_path).map_err(|e| e.to_string())?;
    let mut opts = StatusOptions::new();
    opts.include_untracked(false).include_ignored(false);
    let statuses = repo.statuses(Some(&mut opts)).map_err(|e| e.to_string())?;

    let conflicted_files: Vec<String> = statuses
        .iter()
        .filter(|e| e.status().contains(Status::CONFLICTED))
        .filter_map(|e| e.path().map(|p| p.to_string()))
        .collect();

    Ok(RepoOperationState {
        in_merge,
        in_rebase,
        in_cherry_pick,
        in_revert,
        conflicted_files,
    })
}

/// Read the three versions of a conflicted file (base/ours/theirs) from the index stages.
pub fn get_conflict_file(repo_path: &str, file_path: &str) -> Result<ConflictFile, String> {
    let repo = crate::repo_pool::open_repo(repo_path).map_err(|e| e.to_string())?;
    let index = repo.index().map_err(|e| e.to_string())?;

    let mut base = String::new();
    let mut ours = String::new();
    let mut theirs = String::new();

    // Stage 1 = base, Stage 2 = ours, Stage 3 = theirs
    for entry in index.iter() {
        let entry_path = String::from_utf8_lossy(&entry.path).to_string();
        if entry_path != file_path {
            continue;
        }

        let blob = repo.find_blob(entry.id).map_err(|e| e.to_string())?;
        let content = String::from_utf8_lossy(blob.content()).to_string();

        match entry.flags & 0x3000 {
            // stage bits are bits 12-13
            0x1000 => base = content,  // stage 1
            0x2000 => ours = content,  // stage 2
            0x3000 => theirs = content, // stage 3
            _ => {}
        }
    }

    // Read the current working file (with conflict markers)
    let full_path = Path::new(repo_path).join(file_path);
    let merged = fs::read_to_string(&full_path).unwrap_or_default();

    Ok(ConflictFile {
        path: file_path.to_string(),
        ours,
        theirs,
        base,
        merged,
    })
}

/// Resolve a conflict by writing the resolved content and staging the file.
pub fn resolve_conflict(repo_path: &str, file_path: &str, resolved_content: &str) -> Result<(), String> {
    let full_path = Path::new(repo_path).join(file_path);
    fs::write(&full_path, resolved_content).map_err(|e| e.to_string())?;

    // Stage the resolved file
    let repo = crate::repo_pool::open_repo(repo_path).map_err(|e| e.to_string())?;
    let mut index = repo.index().map_err(|e| e.to_string())?;
    index
        .add_path(Path::new(file_path))
        .map_err(|e| e.to_string())?;
    index.write().map_err(|e| e.to_string())?;

    Ok(())
}

/// Continue the current operation (merge/rebase/cherry-pick).
pub fn continue_operation(repo_path: &str) -> Result<String, String> {
    let state = detect_operation_state(repo_path)?;

    let args = if state.in_rebase {
        vec!["rebase", "--continue"]
    } else if state.in_cherry_pick {
        vec!["cherry-pick", "--continue"]
    } else if state.in_revert {
        vec!["revert", "--continue"]
    } else if state.in_merge {
        // For merge, committing is "continue"
        vec!["commit", "--no-edit"]
    } else {
        return Err("No operation in progress".to_string());
    };

    let output = Command::new("git")
        .current_dir(repo_path)
        .args(&args)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Abort the current operation.
pub fn abort_operation(repo_path: &str) -> Result<String, String> {
    let state = detect_operation_state(repo_path)?;

    let args = if state.in_rebase {
        vec!["rebase", "--abort"]
    } else if state.in_cherry_pick {
        vec!["cherry-pick", "--abort"]
    } else if state.in_revert {
        vec!["revert", "--abort"]
    } else if state.in_merge {
        vec!["merge", "--abort"]
    } else {
        return Err("No operation in progress".to_string());
    };

    let output = Command::new("git")
        .current_dir(repo_path)
        .args(&args)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok("Operation aborted".to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}
