//! Multi-account Git identity support.
//! Reads ~/.ssh/config to discover SSH profiles and matches them to repo remotes.

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

    Ok(GitIdentity {
        name,
        email,
        ssh_host,
        ssh_key,
        account_label,
        remote_url,
        is_correct: true,
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
