//! Git bisect operations for binary search through commit history.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BisectState {
    pub active: bool,
    pub current_hash: String,
    pub current_short_hash: String,
    pub current_message: String,
    pub steps_remaining: Option<usize>,
    pub good_hashes: Vec<String>,
    pub bad_hashes: Vec<String>,
    pub found_hash: Option<String>,
}

/// Check if bisect is in progress.
pub fn get_bisect_state(repo_path: &str) -> Result<BisectState, String> {
    let bisect_log = Path::new(repo_path).join(".git/BISECT_LOG");
    if !bisect_log.exists() {
        return Ok(BisectState {
            active: false,
            current_hash: String::new(),
            current_short_hash: String::new(),
            current_message: String::new(),
            steps_remaining: None,
            good_hashes: vec![],
            bad_hashes: vec![],
            found_hash: None,
        });
    }

    // Get current HEAD info
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["log", "-1", "--format=%H|%h|%s"])
        .output()
        .map_err(|e| e.to_string())?;
    let head_info = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let parts: Vec<&str> = head_info.splitn(3, '|').collect();

    // Parse bisect log for good/bad hashes
    let log_content = std::fs::read_to_string(&bisect_log).unwrap_or_default();
    let mut good_hashes = vec![];
    let mut bad_hashes = vec![];

    for line in log_content.lines() {
        if line.starts_with("# good:") || line.contains("git bisect good") {
            if let Some(hash) = line.split_whitespace().last() {
                good_hashes.push(hash.to_string());
            }
        } else if line.starts_with("# bad:") || line.contains("git bisect bad") {
            if let Some(hash) = line.split_whitespace().last() {
                bad_hashes.push(hash.to_string());
            }
        }
    }

    // Estimate remaining steps
    let remaining = Command::new("git")
        .current_dir(repo_path)
        .args(["bisect", "visualize", "--oneline"])
        .output()
        .ok()
        .map(|o| {
            let count = String::from_utf8_lossy(&o.stdout).lines().count();
            if count > 0 { (count as f64).log2().ceil() as usize } else { 0 }
        });

    Ok(BisectState {
        active: true,
        current_hash: parts.first().unwrap_or(&"").to_string(),
        current_short_hash: parts.get(1).unwrap_or(&"").to_string(),
        current_message: parts.get(2).unwrap_or(&"").to_string(),
        steps_remaining: remaining,
        good_hashes,
        bad_hashes,
        found_hash: None,
    })
}

/// Start a bisect session with a known bad commit and good commit.
pub fn start_bisect(repo_path: &str, bad_hash: &str, good_hash: &str) -> Result<BisectState, String> {
    // Start bisect
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["bisect", "start", bad_hash, good_hash])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    get_bisect_state(repo_path)
}

/// Mark the current commit as good.
pub fn bisect_good(repo_path: &str) -> Result<BisectState, String> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["bisect", "good"])
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Check if bisect is done
    if stdout.contains("is the first bad commit") {
        let mut state = get_bisect_state(repo_path)?;
        // Extract the found hash from output
        if let Some(hash) = stdout.split_whitespace().next() {
            state.found_hash = Some(hash.to_string());
        }
        return Ok(state);
    }

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    get_bisect_state(repo_path)
}

/// Mark the current commit as bad.
pub fn bisect_bad(repo_path: &str) -> Result<BisectState, String> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["bisect", "bad"])
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if stdout.contains("is the first bad commit") {
        let mut state = get_bisect_state(repo_path)?;
        if let Some(hash) = stdout.split_whitespace().next() {
            state.found_hash = Some(hash.to_string());
        }
        return Ok(state);
    }

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    get_bisect_state(repo_path)
}

/// Skip the current commit.
pub fn bisect_skip(repo_path: &str) -> Result<BisectState, String> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["bisect", "skip"])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    get_bisect_state(repo_path)
}

/// End the bisect session and return to original HEAD.
pub fn bisect_reset(repo_path: &str) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["bisect", "reset"])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok("Bisect session ended".to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}
