//! Remotes module for remote repository operations.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use git2::{
    build::CheckoutBuilder, Config, Cred, FetchOptions, PushOptions, RemoteCallbacks, Repository,
};
use serde::{Deserialize, Serialize};

use crate::errors::{GitfastError, GitfastResult};

/// Information about a git remote.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
}

/// Result of a fetch operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchResult {
    pub remote: String,
    pub updated_refs: Vec<String>,
    pub new_refs: Vec<String>,
}

/// Result of a push operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushResult {
    pub remote: String,
    pub branch: String,
    pub success: bool,
    pub message: String,
}

fn open_repo(repo_path: &str) -> Result<Repository, GitfastError> {
    let path = Path::new(repo_path);
    if !path.exists() {
        return Err(GitfastError::RepoNotFound(repo_path.to_string()));
    }
    Repository::open(repo_path).map_err(|e| GitfastError::NotAGitRepo(e.to_string()))
}

/// Returns all configured remotes.
pub async fn list_remotes(repo_path: &str) -> GitfastResult<Vec<RemoteInfo>> {
    let path = repo_path.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;
        let remotes = repo
            .remotes()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let mut result = Vec::new();
        for name in remotes.iter().flatten() {
            let remote = repo
                .find_remote(name)
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            let fetch_url = remote
                .url()
                .unwrap_or("")
                .to_string();
            let push_url = remote
                .pushurl()
                .unwrap_or_else(|| remote.url().unwrap_or(""))
                .to_string();
            result.push(RemoteInfo {
                name: name.to_string(),
                fetch_url,
                push_url,
            });
        }
        Ok(result)
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Fetches from the given remote.
/// Uses SSH agent first, then credential helper for auth.
pub async fn fetch(repo_path: &str, remote_name: &str) -> GitfastResult<FetchResult> {
    let path = repo_path.to_string();
    let remote_name = remote_name.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;
        let config = repo
            .config()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let mut remote = repo
            .find_remote(&remote_name)
            .map_err(|e| GitfastError::RemoteError(e.to_string()))?;

        let updated_refs = Rc::new(RefCell::new(Vec::<String>::new()));
        let new_refs = Rc::new(RefCell::new(Vec::<String>::new()));
        let updated_refs_cb = updated_refs.clone();
        let new_refs_cb = new_refs.clone();

        let mut cb = RemoteCallbacks::new();
        let config_ref = unsafe { &*(&config as *const Config) };
        cb.credentials(move |url, username, allowed_types| {
            let username = username.unwrap_or("git");
            if allowed_types.is_ssh_key() || allowed_types.is_ssh_memory() || allowed_types.is_default() {
                if let Ok(cred) = Cred::ssh_key_from_agent(username) {
                    return Ok(cred);
                }
            }
            if allowed_types.is_user_pass_plaintext() || allowed_types.is_default() {
                if let Ok(cred) = Cred::credential_helper(config_ref, url, Some(username)) {
                    return Ok(cred);
                }
            }
            Err(git2::Error::from_str("authentication failed: try SSH agent or credential helper"))
        });
        cb.transfer_progress(|stats| {
            if stats.total_objects() > 0 && stats.received_objects() < stats.total_objects() {
                eprintln!(
                    "\rFetching: {}/{} objects",
                    stats.received_objects(),
                    stats.total_objects()
                );
            }
            true
        });
        cb.update_tips(move |refname, old_oid, _new_oid| {
            if old_oid.is_zero() {
                new_refs_cb.borrow_mut().push(refname.to_string());
            } else {
                updated_refs_cb.borrow_mut().push(refname.to_string());
            }
            true
        });

        let mut opts = FetchOptions::new();
        opts.remote_callbacks(cb);

        remote
            .fetch(&[] as &[&str], Some(&mut opts), None)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        Ok(FetchResult {
            remote: remote_name,
            updated_refs: Rc::try_unwrap(updated_refs)
                .unwrap_or_else(|r| RefCell::new((*r.borrow()).clone()))
                .into_inner(),
            new_refs: Rc::try_unwrap(new_refs)
                .unwrap_or_else(|r| RefCell::new((*r.borrow()).clone()))
                .into_inner(),
        })
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Fetch + fast-forward merge only. Returns status message.
/// Fails with clear error if fast-forward is not possible.
pub async fn pull(repo_path: &str, remote_name: &str, branch_name: &str) -> GitfastResult<String> {
    let path = repo_path.to_string();
    let remote_name = remote_name.to_string();
    let branch_name = branch_name.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;
        let config = repo
            .config()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let mut remote = repo
            .find_remote(&remote_name)
            .map_err(|e| GitfastError::RemoteError(e.to_string()))?;

        let mut cb = RemoteCallbacks::new();
        let config_ref = unsafe { &*(&config as *const Config) };
        cb.credentials(move |url, username, allowed_types| {
            let username = username.unwrap_or("git");
            if allowed_types.is_ssh_key() || allowed_types.is_ssh_memory() || allowed_types.is_default() {
                if let Ok(cred) = Cred::ssh_key_from_agent(username) {
                    return Ok(cred);
                }
            }
            if allowed_types.is_user_pass_plaintext() || allowed_types.is_default() {
                if let Ok(cred) = Cred::credential_helper(config_ref, url, Some(username)) {
                    return Ok(cred);
                }
            }
            Err(git2::Error::from_str("authentication failed: try SSH agent or credential helper"))
        });
        cb.transfer_progress(|stats| {
            if stats.total_objects() > 0 && stats.received_objects() < stats.total_objects() {
                eprintln!(
                    "\rFetching: {}/{} objects",
                    stats.received_objects(),
                    stats.total_objects()
                );
            }
            true
        });

        let mut opts = FetchOptions::new();
        opts.remote_callbacks(cb);

        remote
            .fetch(&[branch_name.as_str()], Some(&mut opts), None)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        let fetch_head = repo
            .find_reference("FETCH_HEAD")
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let fetch_commit = repo
            .reference_to_annotated_commit(&fetch_head)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        let (analysis, _) = repo
            .merge_analysis(&[&fetch_commit])
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        if analysis.is_fast_forward() || analysis.is_unborn() {
            let refname = format!("refs/heads/{}", branch_name);
            match repo.find_reference(&refname) {
                Ok(mut lb) => {
                    let msg = format!("Fast-Forward: Setting {} to {}", refname, fetch_commit.id());
                    lb.set_target(fetch_commit.id(), &msg)
                        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
                    repo.set_head(&refname)
                        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
                    repo.checkout_head(Some(
                        CheckoutBuilder::new().force(),
                    ))
                        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
                    Ok(format!(
                        "Fast-forward: {} updated to {}",
                        branch_name, fetch_commit.id()
                    ))
                }
                Err(_) => {
                    repo.reference(
                        &refname,
                        fetch_commit.id(),
                        true,
                        &format!("Setting {} to {}", branch_name, fetch_commit.id()),
                    )
                    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
                    repo.set_head(&refname)
                        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
                    repo.checkout_head(Some(
                        CheckoutBuilder::new()
                            .allow_conflicts(true)
                            .conflict_style_merge(true)
                            .force(),
                    ))
                        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
                    Ok(format!(
                        "Created branch {} at {}",
                        branch_name, fetch_commit.id()
                    ))
                }
            }
        } else if analysis.is_up_to_date() {
            Ok(format!("Already up to date with {}/{}", remote_name, branch_name))
        } else if analysis.is_normal() {
            Err(GitfastError::GitOperationFailed(
                "fast-forward not possible: branches have diverged. Use merge or rebase manually."
                    .to_string(),
            ))
        } else {
            Err(GitfastError::GitOperationFailed(
                "pull failed: cannot fast-forward. Branches have diverged or merge is required."
                    .to_string(),
            ))
        }
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Pushes a branch to the remote.
pub async fn push(
    repo_path: &str,
    remote_name: &str,
    branch_name: &str,
    force: bool,
) -> GitfastResult<PushResult> {
    let path = repo_path.to_string();
    let remote_name = remote_name.to_string();
    let branch_name = branch_name.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;
        let config = repo
            .config()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let mut remote = repo
            .find_remote(&remote_name)
            .map_err(|e| GitfastError::RemoteError(e.to_string()))?;

        let push_msg = Rc::new(RefCell::new(None::<String>));
        let push_success = Rc::new(RefCell::new(true));
        let push_msg_cb = push_msg.clone();
        let push_success_cb = push_success.clone();

        let mut cb = RemoteCallbacks::new();
        let config_ref = unsafe { &*(&config as *const Config) };
        cb.credentials(move |url, username, allowed_types| {
            let username = username.unwrap_or("git");
            if allowed_types.is_ssh_key() || allowed_types.is_ssh_memory() || allowed_types.is_default() {
                if let Ok(cred) = Cred::ssh_key_from_agent(username) {
                    return Ok(cred);
                }
            }
            if allowed_types.is_user_pass_plaintext() || allowed_types.is_default() {
                if let Ok(cred) = Cred::credential_helper(config_ref, url, Some(username)) {
                    return Ok(cred);
                }
            }
            Err(git2::Error::from_str("authentication failed: try SSH agent or credential helper"))
        });
        cb.push_update_reference(move |_refname, status| {
            if let Some(msg) = status {
                *push_success_cb.borrow_mut() = false;
                *push_msg_cb.borrow_mut() = Some(msg.to_string());
                Err(git2::Error::from_str(msg))
            } else {
                *push_msg_cb.borrow_mut() = Some("ok".to_string());
                Ok(())
            }
        });

        let mut opts = PushOptions::new();
        opts.remote_callbacks(cb);

        let refspec = if force {
            format!("+refs/heads/{}:refs/heads/{}", branch_name, branch_name)
        } else {
            format!("refs/heads/{}:refs/heads/{}", branch_name, branch_name)
        };

        let result = remote.push(&[refspec.as_str()], Some(&mut opts));

        let (success, message) = match result {
            Ok(()) => (
                *push_success.borrow(),
                push_msg.borrow().clone().unwrap_or_else(|| "ok".to_string()),
            ),
            Err(e) => (false, e.to_string()),
        };

        Ok(PushResult {
            remote: remote_name,
            branch: branch_name,
            success,
            message,
        })
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}

/// Returns all remotes as a JSON string.
pub async fn list_remotes_json(repo_path: &str) -> GitfastResult<String> {
    let remotes = list_remotes(repo_path).await?;
    serde_json::to_string_pretty(&remotes)
        .map_err(|e| GitfastError::SerializationError(e.to_string()))
}
