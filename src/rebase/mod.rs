//! Interactive rebase operations.

use std::fs;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebaseTodoItem {
    pub action: String,
    pub hash: String,
    pub short_hash: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebaseState {
    pub in_progress: bool,
    pub current_step: usize,
    pub total_steps: usize,
    pub todo_items: Vec<RebaseTodoItem>,
    pub head_name: Option<String>,
    pub onto: Option<String>,
}

/// Get the current rebase state by reading .git/rebase-merge/ files.
pub fn get_rebase_state(repo_path: &str) -> Result<RebaseState, String> {
    let rebase_dir = Path::new(repo_path).join(".git/rebase-merge");

    if !rebase_dir.exists() {
        let rebase_apply = Path::new(repo_path).join(".git/rebase-apply");
        if !rebase_apply.exists() {
            return Ok(RebaseState {
                in_progress: false,
                current_step: 0,
                total_steps: 0,
                todo_items: vec![],
                head_name: None,
                onto: None,
            });
        }
        // rebase-apply is for non-interactive rebase
        return Ok(RebaseState {
            in_progress: true,
            current_step: 0,
            total_steps: 0,
            todo_items: vec![],
            head_name: None,
            onto: None,
        });
    }

    let head_name = fs::read_to_string(rebase_dir.join("head-name"))
        .ok()
        .map(|s| s.trim().strip_prefix("refs/heads/").unwrap_or(s.trim()).to_string());

    let onto = fs::read_to_string(rebase_dir.join("onto"))
        .ok()
        .map(|s| s.trim().to_string());

    // Parse the todo file
    let todo_content = fs::read_to_string(rebase_dir.join("git-rebase-todo"))
        .unwrap_or_default();

    let todo_items: Vec<RebaseTodoItem> = todo_content
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let parts: Vec<&str> = l.splitn(3, ' ').collect();
            if parts.len() < 3 {
                return None;
            }
            Some(RebaseTodoItem {
                action: parts[0].to_string(),
                hash: String::new(),
                short_hash: parts[1].to_string(),
                message: parts[2].to_string(),
            })
        })
        .collect();

    // Parse done file to count completed steps
    let done_content = fs::read_to_string(rebase_dir.join("done"))
        .unwrap_or_default();
    let done_count = done_content
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .count();

    let total = done_count + todo_items.len();

    Ok(RebaseState {
        in_progress: true,
        current_step: done_count,
        total_steps: total,
        todo_items,
        head_name,
        onto,
    })
}

/// Get the todo list for a rebase range (commits from HEAD that are not on `onto`).
pub fn get_rebase_todo_for_range(repo_path: &str, onto: &str) -> Result<Vec<RebaseTodoItem>, String> {
    let range = format!("{}..HEAD", onto);
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["log", "--reverse", "--format=%H|%h|%s", &range])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let items: Vec<RebaseTodoItem> = stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let parts: Vec<&str> = l.splitn(3, '|').collect();
            RebaseTodoItem {
                action: "pick".to_string(),
                hash: parts.first().unwrap_or(&"").to_string(),
                short_hash: parts.get(1).unwrap_or(&"").to_string(),
                message: parts.get(2).unwrap_or(&"").to_string(),
            }
        })
        .collect();

    Ok(items)
}

/// Start an interactive rebase using the GIT_SEQUENCE_EDITOR trick.
/// Uses full 40-char hashes to avoid ambiguity in large repos.
pub fn start_interactive_rebase(repo_path: &str, onto: &str, todo_json: &str) -> Result<String, String> {
    let items: Vec<RebaseTodoItem> = serde_json::from_str(todo_json)
        .map_err(|e| format!("Invalid todo JSON: {}", e))?;

    // H13: always use the full hash (item.hash), not short_hash
    let todo_content: String = items
        .iter()
        .map(|item| {
            let hash = if item.hash.is_empty() { &item.short_hash } else { &item.hash };
            format!("{} {} {}", item.action, hash, item.message)
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Write to temp file
    let temp_dir = std::env::temp_dir();
    let todo_file = temp_dir.join("gitaxon_rebase_todo");
    fs::write(&todo_file, &todo_content).map_err(|e| e.to_string())?;

    let todo_path = todo_file.to_string_lossy().to_string();

    // Use GIT_SEQUENCE_EDITOR to replace the interactive editor
    // The command copies our todo file over the one git generates
    let editor_cmd = format!("cp {} ", todo_path);

    let output = Command::new("git")
        .current_dir(repo_path)
        .env("GIT_SEQUENCE_EDITOR", &editor_cmd)
        .args(["rebase", "-i", onto])
        .output()
        .map_err(|e| e.to_string())?;

    // Clean up temp file
    let _ = fs::remove_file(&todo_file);

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        // Conflicts are expected during rebase — not an error
        if stderr.contains("CONFLICT") || stderr.contains("could not apply") {
            Ok(format!("Rebase paused: {}", stderr))
        } else {
            Err(stderr)
        }
    }
}

/// Continue the rebase after resolving conflicts.
/// If `new_message` is provided (for a reword step), it is written via a temp GIT_EDITOR script.
pub fn continue_rebase(repo_path: &str, new_message: Option<&str>) -> Result<String, String> {
    let editor_script = if let Some(msg) = new_message {
        // H12: for reword, write the message to a temp file and use a script to copy it into git's editor target
        let tmp = std::env::temp_dir().join("gitaxon_reword_msg");
        fs::write(&tmp, msg).map_err(|e| e.to_string())?;
        format!("cp {} \"$1\"", tmp.to_string_lossy())
    } else {
        // For all other steps, use the `true` binary to accept the existing message without opening an editor
        "true".to_string()
    };

    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["rebase", "--continue"])
        .env("GIT_EDITOR", &editor_script)
        .env("GIT_SEQUENCE_EDITOR", "true")
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Abort the rebase.
pub fn abort_rebase(repo_path: &str) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["rebase", "--abort"])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok("Rebase aborted".to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Skip the current commit during rebase.
pub fn skip_rebase(repo_path: &str) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["rebase", "--skip"])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}
