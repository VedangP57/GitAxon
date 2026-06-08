//! Pooled git2::Repository handles to avoid repeated Repository::open() calls.
//!
//! Every module was independently calling Repository::open() on each operation.
//! This module caches the handle per repo path, avoiding ~2-5ms of disk I/O per call.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use git2::Repository;
use once_cell::sync::Lazy;

use crate::errors::GitfastError;

type RepoHandle = Arc<Mutex<Repository>>;

static POOL: Lazy<Mutex<HashMap<String, RepoHandle>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Get a pooled Repository handle. Opens the repo on first access, reuses on subsequent calls.
pub fn get_repo(repo_path: &str) -> Result<RepoHandle, GitfastError> {
    let path = Path::new(repo_path);
    if !path.exists() {
        return Err(GitfastError::RepoNotFound(repo_path.to_string()));
    }

    let mut pool = POOL
        .lock()
        .map_err(|_| GitfastError::GitOperationFailed("repo pool lock poisoned".into()))?;

    if let Some(handle) = pool.get(repo_path) {
        return Ok(Arc::clone(handle));
    }

    let repo = Repository::open(repo_path)
        .map_err(|e| GitfastError::NotAGitRepo(e.to_string()))?;
    let handle = Arc::new(Mutex::new(repo));
    pool.insert(repo_path.to_string(), Arc::clone(&handle));
    Ok(handle)
}

/// Open a fresh (non-pooled) Repository. Use when you need a short-lived handle
/// that won't contend with other operations (e.g., inside spawn_blocking).
pub fn open_repo(repo_path: &str) -> Result<Repository, GitfastError> {
    let path = Path::new(repo_path);
    if !path.exists() {
        return Err(GitfastError::RepoNotFound(repo_path.to_string()));
    }
    Repository::open(repo_path).map_err(|e| GitfastError::NotAGitRepo(e.to_string()))
}

/// Remove a cached handle (e.g., when switching repos).
pub fn invalidate(repo_path: &str) {
    if let Ok(mut pool) = POOL.lock() {
        pool.remove(repo_path);
    }
}

/// Clear all cached handles.
pub fn clear() {
    if let Ok(mut pool) = POOL.lock() {
        pool.clear();
    }
}
