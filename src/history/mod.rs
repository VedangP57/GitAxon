//! File history module — git log for individual files.

use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHistoryEntry {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    pub timestamp: i64,
    pub additions: i64,
    pub deletions: i64,
}

/// Returns the commit history for a single file, following renames.
pub fn file_history(
    repo_path: &str,
    file_path: &str,
    limit: usize,
) -> Result<Vec<FileHistoryEntry>, String> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args([
            "log",
            "--follow",
            "--format=%H|%h|%s|%an|%ae|%ct",
            "--numstat",
            &format!("-{}", limit),
            "--",
            file_path,
        ])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();
    let mut lines = stdout.lines().peekable();

    while let Some(line) = lines.next() {
        if line.is_empty() {
            continue;
        }

        // Format line: hash|short_hash|message|author_name|author_email|timestamp
        let parts: Vec<&str> = line.splitn(6, '|').collect();
        if parts.len() < 6 {
            continue;
        }

        let hash = parts[0].to_string();
        let short_hash = parts[1].to_string();
        let message = parts[2].to_string();
        let author_name = parts[3].to_string();
        let author_email = parts[4].to_string();
        let timestamp = parts[5].parse::<i64>().unwrap_or(0);

        // Next non-empty line(s) may be numstat: additions\tdeletions\tfilename
        let mut additions: i64 = 0;
        let mut deletions: i64 = 0;

        while let Some(stat_line) = lines.peek() {
            if stat_line.is_empty() {
                lines.next(); // consume blank line
                break;
            }
            // numstat format: "10\t5\tpath/to/file"
            let stat_parts: Vec<&str> = stat_line.split('\t').collect();
            if stat_parts.len() >= 2 {
                additions += stat_parts[0].parse::<i64>().unwrap_or(0);
                deletions += stat_parts[1].parse::<i64>().unwrap_or(0);
            }
            lines.next();
        }

        entries.push(FileHistoryEntry {
            hash,
            short_hash,
            message,
            author_name,
            author_email,
            timestamp,
            additions,
            deletions,
        });
    }

    Ok(entries)
}

/// Returns the diff of a specific file at a specific commit.
pub fn file_diff_at_commit(
    repo_path: &str,
    file_path: &str,
    commit_hash: &str,
) -> Result<Vec<crate::diff::DiffFile>, String> {
    let diff_range = format!("{}^..{}", commit_hash, commit_hash);
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["diff", "--no-color", &diff_range, "--", file_path])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        // First commit has no parent — use --root
        let output2 = Command::new("git")
            .current_dir(repo_path)
            .args([
                "diff-tree",
                "--patch",
                "--no-color",
                "--root",
                commit_hash,
                "--",
                file_path,
            ])
            .output()
            .map_err(|e| e.to_string())?;
        let patch = String::from_utf8_lossy(&output2.stdout);
        return Ok(crate::diff::parse_unified_diff(&patch));
    }

    let patch = String::from_utf8_lossy(&output.stdout);
    Ok(crate::diff::parse_unified_diff(&patch))
}
