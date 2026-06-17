//! Stash operations.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StashEntry {
    pub index: usize,
    pub name: String,
    pub message: String,
    pub branch: String,
    pub date: String,
    pub hash: String,
}

pub fn list_stashes(repo_path: &str) -> Result<Vec<StashEntry>, String> {
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .args(["stash", "list", "--format=%gd|%s|%H|%ci"])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let entries = stdout
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.is_empty())
        .map(|(i, line)| {
            let parts: Vec<&str> = line.splitn(4, '|').collect();
            let name = parts.first().unwrap_or(&"").to_string();
            let message = parts.get(1).unwrap_or(&"").to_string();
            let hash = parts.get(2).unwrap_or(&"").to_string();
            let date = parts.get(3).unwrap_or(&"").to_string();
            let branch = message
                .strip_prefix("WIP on ")
                .and_then(|s| s.split(':').next())
                .unwrap_or("unknown")
                .to_string();

            StashEntry {
                index: i,
                name,
                message,
                branch,
                date,
                hash,
            }
        })
        .collect();

    Ok(entries)
}

pub fn stash_push(repo_path: &str, message: &str) -> Result<String, String> {
    let output = if message.trim().is_empty() {
        std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["stash", "push", "--include-untracked"])
            .output()
            .map_err(|e| e.to_string())?
    } else {
        std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["stash", "push", "--include-untracked", "-m", message])
            .output()
            .map_err(|e| e.to_string())?
    };

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

pub fn stash_pop(repo_path: &str, index: usize) -> Result<String, String> {
    let stash_ref = format!("stash@{{{}}}", index);
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .args(["stash", "pop", &stash_ref])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(format!("Applied stash@{{{}}}", index))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

pub fn stash_apply(repo_path: &str, index: usize) -> Result<String, String> {
    let stash_ref = format!("stash@{{{}}}", index);
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .args(["stash", "apply", &stash_ref])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(format!("Applied stash@{{{}}}", index))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

pub fn stash_drop(repo_path: &str, index: usize) -> Result<String, String> {
    let stash_ref = format!("stash@{{{}}}", index);
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .args(["stash", "drop", &stash_ref])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(format!("Dropped stash@{{{}}}", index))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Returns the diff of a stash entry as a list of DiffFile objects.
pub fn stash_show(repo_path: &str, index: usize) -> Result<Vec<crate::diff::DiffFile>, String> {
    let stash_ref = format!("stash@{{{}}}", index);
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .args(["stash", "show", "-p", "--no-color", &stash_ref])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let patch = String::from_utf8_lossy(&output.stdout);
    Ok(crate::diff::parse_unified_diff(&patch))
}

pub fn stash_branch(repo_path: &str, index: usize, branch_name: &str) -> Result<String, String> {
    let stash_ref = format!("stash@{{{}}}", index);
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .args(["stash", "branch", branch_name, &stash_ref])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(format!("Created branch {} from stash", branch_name))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}
