//! Multi-account Git identity support.
//! Reads ~/.ssh/config to discover SSH profiles and matches them to repo remotes.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

static SSH_USERNAME_CACHE: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Run `ssh -T git@{host}` and parse GitHub username from "Hi username!" response.
pub fn get_github_username_for_host(host_alias: &str) -> Option<String> {
    let output = std::process::Command::new("ssh")
        .arg("-T")
        .arg("-o")
        .arg("StrictHostKeyChecking=no")
        .arg("-o")
        .arg("ConnectTimeout=5")
        .arg(format!("git@{}", host_alias))
        .output()
        .ok()?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let response = format!("{}{}", stdout, stderr);

    if let Some(start) = response.find("Hi ") {
        let rest = &response[start + 3..];
        if let Some(end) = rest.find('!') {
            return Some(rest[..end].trim().to_string());
        }
    }
    None
}

pub fn get_github_username_cached(host_alias: &str) -> Option<String> {
    {
        let cache = SSH_USERNAME_CACHE.lock().unwrap();
        if let Some(username) = cache.get(host_alias) {
            return Some(username.clone());
        }
    }

    let username = get_github_username_for_host(host_alias)?;

    {
        let mut cache = SSH_USERNAME_CACHE.lock().unwrap();
        cache.insert(host_alias.to_string(), username.clone());
    }

    Some(username)
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct GitIdentity {
    pub name: String,
    pub email: String,
    pub ssh_host: String,       // "github.com" or "github-personal"
    pub ssh_key: String,        // "~/.ssh/id_ed25519" etc
    pub account_label: String,  // "Office" or "Personal" or host alias
    pub remote_url: String,
    pub is_correct: bool,       // does remote host match local config
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct SshProfile {
    pub host_alias: String,     // "github.com" or "github-personal"
    pub hostname: String,       // always "github.com"
    pub identity_file: String,  // "~/.ssh/id_ed25519"
    pub label: String,          // human readable label
}

/// Read ~/.ssh/config and parse all Host blocks
pub fn get_ssh_profiles() -> Vec<SshProfile> {
    let config_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".ssh/config");

    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut profiles = vec![];
    let mut current_host = String::new();
    let mut current_hostname = String::new();
    let mut current_identity = String::new();

    for line in content.lines() {
        let line = line.trim();

        if line.starts_with("Host ") && !line.starts_with("HostName") {
            // Save previous block
            if !current_host.is_empty() && !current_identity.is_empty() {
                let label = if current_host == "github.com" {
                    "Office (github.com)".to_string()
                } else if current_host.contains("personal") {
                    "Personal".to_string()
                } else {
                    current_host.clone()
                };
                profiles.push(SshProfile {
                    host_alias: current_host.clone(),
                    hostname: current_hostname.clone(),
                    identity_file: current_identity.clone(),
                    label,
                });
            }
            current_host = line[5..].trim().to_string();
            current_hostname = String::new();
            current_identity = String::new();
        } else if line.starts_with("HostName ") {
            current_hostname = line[9..].trim().to_string();
        } else if line.starts_with("IdentityFile ") {
            current_identity = line[13..].trim().to_string();
        }
    }

    // Save last block
    if !current_host.is_empty() && !current_identity.is_empty() {
        let label = if current_host == "github.com" {
            "Office (github.com)".to_string()
        } else if current_host.contains("personal") {
            "Personal".to_string()
        } else {
            current_host.clone()
        };
        profiles.push(SshProfile {
            host_alias: current_host,
            hostname: current_hostname,
            identity_file: current_identity,
            label,
        });
    }

    profiles
}

/// Get current identity for a repo
pub fn get_repo_identity(repo_path: &str) -> Result<GitIdentity, String> {
    let repo = git2::Repository::open(repo_path).map_err(|e| e.to_string())?;

    // Get local git config (falls back to global)
    let config = repo.config().map_err(|e| e.to_string())?;

    let name = config.get_string("user.name").unwrap_or_default();
    let email = config.get_string("user.email").unwrap_or_default();

    // Get remote URL
    let remote_url = repo
        .find_remote("origin")
        .ok()
        .and_then(|r| r.url().map(|u| u.to_string()))
        .unwrap_or_default();

    // Extract SSH host from remote URL
    // git@github.com:org/repo.git → "github.com"
    // git@github-personal:user/repo.git → "github-personal"
    let ssh_host = if remote_url.starts_with("git@") {
        remote_url
            .trim_start_matches("git@")
            .split(':')
            .next()
            .unwrap_or("github.com")
            .to_string()
    } else {
        "github.com".to_string()
    };

    // Find matching SSH profile
    let profiles = get_ssh_profiles();
    let profile = profiles.iter().find(|p| p.host_alias == ssh_host);

    let ssh_key = profile
        .map(|p| p.identity_file.clone())
        .unwrap_or_default();

    let account_label = profile
        .map(|p| p.label.clone())
        .unwrap_or_else(|| ssh_host.clone());

    // Get GitHub username for this SSH host (cached)
    let github_username = get_github_username_cached(&ssh_host);

    let display_email = github_username
        .as_ref()
        .map(|u| format!("{}  (github.com)", u))
        .unwrap_or_else(|| email.clone());

    let display_name = github_username
        .as_ref()
        .map(|u| u.clone())
        .unwrap_or_else(|| name.clone());

    let is_office_key = ssh_key.contains("id_ed25519") 
        && !ssh_key.contains("vedangp57");
    let is_personal_key = ssh_key.contains("vedangp57");

    // Known office org names — read from git remote URL
    let known_office_orgs = vec![
        "elvee-jewels",
        "Sarvadhi-Solutions", 
        "sarvadhi",
    ];

    let remote_org = remote_url
        .split(':')
        .nth(1)
        .unwrap_or("")
        .split('/')
        .next()
        .unwrap_or("");

    let is_office_repo = known_office_orgs.iter()
        .any(|org| remote_org.to_lowercase()
            .contains(&org.to_lowercase()));

    // is_correct = key type matches repo type
    let is_correct = if is_office_repo {
        is_office_key  // office repo should use office key
    } else if is_personal_key {
        true  // personal key on personal repo = correct
    } else {
        true  // unknown repo, assume correct
    };

    Ok(GitIdentity {
        name: display_name,
        email: display_email,
        ssh_host,
        ssh_key,
        account_label,
        remote_url,
        is_correct,
    })
}

/// Switch account for a repo.
/// Changes local git config user.name/email
/// AND updates remote URL to use correct SSH host
pub fn switch_repo_identity(
    repo_path: &str,
    ssh_host_alias: &str,
    name: &str,
    email: &str,
) -> Result<(), String> {
    let repo = git2::Repository::open(repo_path).map_err(|e| e.to_string())?;

    // Update local git config - open repo config directly for writing
    let config_path = repo.path().join("config");
    let mut config = git2::Config::open(&config_path).map_err(|e| e.to_string())?;
    config
        .set_str("user.name", name)
        .map_err(|e| e.to_string())?;
    config
        .set_str("user.email", email)
        .map_err(|e| e.to_string())?;

    // Update remote URL to use correct SSH host alias
    // git@github.com:org/repo.git → git@github-personal:org/repo.git
    if let Ok(remote) = repo.find_remote("origin") {
        if let Some(url) = remote.url() {
            let new_url = if url.starts_with("git@") {
                let after_at = url.trim_start_matches("git@");
                let colon_pos = after_at.find(':').unwrap_or(0);
                let path_part = &after_at[colon_pos..];
                format!("git@{}{}", ssh_host_alias, path_part)
            } else {
                url.to_string()
            };

            repo.remote_set_url("origin", &new_url)
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}
