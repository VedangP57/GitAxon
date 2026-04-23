use notify::event::EventKind;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use git2::{Status, StatusOptions};
use once_cell::sync::Lazy;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct FileStatus {
    pub path: String,
    /// Status code: "M", "A", "D", "WM", "WD", "?"
    pub status: String,
    pub staged: bool,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct StatusPatch {
    pub added: Vec<FileStatus>,
    pub removed: Vec<String>,
    pub changed: Vec<FileStatus>,
}

// Global status cache keyed by repo path
pub static REPO_STATUS: Lazy<Arc<RwLock<HashMap<String, Vec<FileStatus>>>>> =
    Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));

// Previous snapshot for fast patch computation: per-repo path -> (status_code, staged)
type StatusSnapshot = HashMap<String, (String, bool)>;
static PREV_STATUS: Lazy<Mutex<HashMap<String, StatusSnapshot>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

static WATCHER: Mutex<Option<RecommendedWatcher>> = Mutex::new(None);
static IS_RUNNING: AtomicBool = AtomicBool::new(false);

/// Pre-compiled set of path fragments to skip in watcher events.
/// Checked once at module load instead of per-event string matching.
static SKIP_FRAGMENTS: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
        "node_modules", "/.git/objects/", "/.git/logs/", "/.git/gc.pid",
        "/.next/", "/dist/", "/target/", "/.turbo/", "/__pycache__/",
    ]
});

static SKIP_SUFFIXES: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![".lock", ".log", ".tmp", "~", ".swp", ".DS_Store", "4913"]
});

#[inline]
fn should_skip_path(p: &str) -> bool {
    SKIP_SUFFIXES.iter().any(|s| p.ends_with(s))
        || SKIP_FRAGMENTS.iter().any(|f| p.contains(f))
}

fn compute_full_status(repo_path: &str) -> Result<Vec<FileStatus>, String> {
    let repo = crate::repo_pool::open_repo(repo_path).map_err(|e| e.to_string())?;

    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(false)
        .include_ignored(false)
        .exclude_submodules(true);

    let statuses = repo.statuses(Some(&mut opts)).map_err(|e| e.to_string())?;

    let mut results: Vec<FileStatus> = statuses
        .iter()
        .flat_map(|e| {
            let s = e.status();
            let path = e.path().unwrap_or("").to_string();
            if path.is_empty() {
                return vec![];
            }

            let mut entries = Vec::new();

            // Conflicted files get a single entry
            if s.contains(Status::CONFLICTED) {
                entries.push(FileStatus {
                    path,
                    status: "U".to_string(),
                    staged: false,
                });
                return entries;
            }

            // Staged (index) statuses
            if s.intersects(
                Status::INDEX_NEW | Status::INDEX_MODIFIED | Status::INDEX_DELETED | Status::INDEX_RENAMED | Status::INDEX_TYPECHANGE,
            ) {
                let code = if s.contains(Status::INDEX_NEW) {
                    "A"
                } else if s.contains(Status::INDEX_DELETED) {
                    "D"
                } else if s.contains(Status::INDEX_RENAMED) {
                    "R"
                } else if s.contains(Status::INDEX_TYPECHANGE) {
                    "T"
                } else {
                    "M"
                };
                entries.push(FileStatus {
                    path: path.clone(),
                    status: code.to_string(),
                    staged: true,
                });
            }

            // Unstaged (worktree) statuses
            if s.intersects(Status::WT_NEW | Status::WT_MODIFIED | Status::WT_DELETED | Status::WT_RENAMED | Status::WT_TYPECHANGE) {
                let code = if s.contains(Status::WT_NEW) {
                    "?"
                } else if s.contains(Status::WT_DELETED) {
                    "WD"
                } else if s.contains(Status::WT_RENAMED) {
                    "WR"
                } else {
                    "WM"
                };
                entries.push(FileStatus {
                    path,
                    status: code.to_string(),
                    staged: false,
                });
            }

            entries
        })
        .collect();

    // Stable ordering for deterministic diffs
    results.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(results)
}

pub fn init_status(repo_path: &str) -> Result<(), String> {
    let status = compute_full_status(repo_path)?;
    let mut state = REPO_STATUS
        .write()
        .map_err(|_e: std::sync::PoisonError<std::sync::RwLockWriteGuard<'_, HashMap<String, Vec<FileStatus>>>>| {
            "failed to lock REPO_STATUS for write".to_string()
        })?;
    state.insert(repo_path.to_string(), status);
    Ok(())
}

/// Initialize the previous status snapshot so that the first patch is empty.
pub fn init_prev_status(repo_path: &str) -> Result<(), String> {
    let current = compute_full_status(repo_path)?;
    set_prev_status(repo_path, &current);
    log::debug!(
        "Initialized status snapshot for {} ({} files)",
        repo_path,
        current.len()
    );
    Ok(())
}

/// Get full status from git (always fresh) and update PREV_STATUS snapshot.
/// PREV_STATUS is ONLY used by the watcher for diff detection; get_status must never
/// compare against it—this just keeps the watcher's baseline in sync with the UI.
pub fn get_full_status_and_sync_snapshot(repo_path: &str) -> Result<Vec<FileStatus>, String> {
    let entries = compute_full_status(repo_path)?;
    set_prev_status(repo_path, &entries);
    Ok(entries)
}

/// Update PREV_STATUS with the given entries. Used after get_status so the watcher
/// has an accurate baseline for diff detection.
fn set_prev_status(repo_path: &str, entries: &[FileStatus]) {
    let Ok(mut all_prev) = PREV_STATUS.lock() else { return };
    let snapshot = all_prev
        .entry(repo_path.to_string())
        .or_insert_with(HashMap::new);
    snapshot.clear();
    for f in entries {
        snapshot.insert(f.path.clone(), (f.status.clone(), f.staged));
    }
}

/// Compute a minimal status patch by diffing the current snapshot against PREV_STATUS.
pub fn update_status(repo_path: &str) -> Result<StatusPatch, String> {
    let new_status = compute_full_status(repo_path)?;

    // Build current snapshot
    let mut current: StatusSnapshot = HashMap::new();
    for f in &new_status {
        current.insert(f.path.clone(), (f.status.clone(), f.staged));
    }

    let mut all_prev = PREV_STATUS.lock()
        .map_err(|_| "Failed to lock PREV_STATUS".to_string())?;
    let prev_snapshot = all_prev
        .entry(repo_path.to_string())
        .or_insert_with(HashMap::new);

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();

    // Files in current but not in prev = added
    for (path, (status, staged)) in &current {
        match prev_snapshot.get(path) {
            None => added.push(FileStatus {
                path: path.clone(),
                status: status.clone(),
                staged: *staged,
            }),
            Some((prev_status, prev_staged)) => {
                if status != prev_status || staged != prev_staged {
                    changed.push(FileStatus {
                        path: path.clone(),
                        status: status.clone(),
                        staged: *staged,
                    });
                }
            }
        }
    }

    // Files in prev but not in current = removed
    for path in prev_snapshot.keys() {
        if !current.contains_key(path) {
            removed.push(path.clone());
        }
    }

    // Update previous snapshot
    *prev_snapshot = current;

    Ok(StatusPatch {
        added,
        removed,
        changed,
    })
}

fn watch_recursive_limited(
    watcher: &mut RecommendedWatcher,
    dir: &Path,
    current_depth: usize,
    max_depth: usize,
    skip: &[&str],
) {
    if current_depth > max_depth {
        return;
    }

    let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if skip.contains(&name) {
        log::debug!("Skipping heavy dir: {:?}", dir);
        return;
    }
    if name.starts_with('.') && name != ".git" {
        return;
    }

    match watcher.watch(dir, RecursiveMode::NonRecursive) {
        Ok(_) => log::trace!("Watching: {:?}", dir),
        Err(e) => {
            log::warn!("Failed to watch {:?}: {}", dir, e);
            return;
        }
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                watch_recursive_limited(watcher, &path, current_depth + 1, max_depth, skip);
            }
        }
    }
}

pub fn start_watching<F>(repo_path: &str, on_change: F) -> Result<(), String>
where
    F: Fn() + Send + Sync + 'static,
{
    let mut handle = WATCHER.lock()
        .map_err(|_| "Failed to lock WATCHER".to_string())?;
    *handle = None;
    IS_RUNNING.store(false, Ordering::Relaxed);

    let last_worktree = std::sync::Arc::new(Mutex::new(Instant::now() - Duration::from_secs(10)));
    let last_gitstate = std::sync::Arc::new(Mutex::new(Instant::now() - Duration::from_secs(10)));

    let last_worktree2 = last_worktree.clone();
    let last_gitstate2 = last_gitstate.clone();

    // Canonicalize repo path for macOS FSEvents reliability
    let real_repo_path = std::fs::canonicalize(repo_path)
        .unwrap_or_else(|_| Path::new(repo_path).to_path_buf());
    let repo_path_str = real_repo_path.to_string_lossy().to_string();
    log::debug!("Canonical path: {}", repo_path_str);

    let config = Config::default().with_poll_interval(Duration::from_millis(50));

    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            if !IS_RUNNING.load(Ordering::Relaxed) {
                return;
            }

            let Ok(event) = res else { return };

            // LOG EVERYTHING first for diagnosis
            log::trace!(
                "RAW event: {:?} paths: {:?}",
                event.kind, event.paths
            );

            // Accept ALL modify/create/remove events
            let is_change = matches!(
                event.kind,
                EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
            );
            if !is_change {
                return;
            }

            for path in &event.paths {
                let p = path.to_string_lossy();

                if should_skip_path(&p) {
                    continue;
                }

                // Any .git state change (HEAD/index/refs) or non-.git file change
                // collapses to a single on_change() trigger. Rust status diffing
                // will figure out what actually changed.
                let is_git_state = p.contains("/.git/HEAD")
                    || p.contains("/.git/index")
                    || p.contains("/.git/refs/heads/")
                    || p.contains("/.git/refs/remotes/")
                    || p.contains("/.git/packed-refs")
                    || p.contains("/.git/COMMIT_EDITMSG");

                let is_worktree = !p.contains("/.git/");

                if is_git_state {
                    let Ok(mut last) = last_gitstate2.lock() else { return };
                    // Fast feedback for index/HEAD changes; keep a tiny debounce
                    // just to coalesce rapid successive writes.
                    if last.elapsed() > Duration::from_millis(50) {
                        *last = Instant::now();
                        drop(last);
                        log::debug!("Git state change: {}", p);
                        on_change();
                    }
                    return;
                }

                if is_worktree {
                    let Ok(mut last) = last_worktree2.lock() else { return };
                    if last.elapsed() > Duration::from_millis(50) {
                        *last = Instant::now();
                        drop(last);

                        log::debug!("Source file changed: {}", p);
                        on_change();
                    }
                    return;
                }
            }
        },
        config,
    )
    .map_err(|e| e.to_string())?;

    // Watch specific .git files and refs only; never watch .git/objects or logs.
    let git_dir = real_repo_path.join(".git");
    if git_dir.exists() {
        // HEAD - branch switches
        let head = git_dir.join("HEAD");
        if head.exists() {
            let _ = watcher.watch(&head, RecursiveMode::NonRecursive);
        }

        // index - staging changes
        let index = git_dir.join("index");
        if index.exists() {
            let _ = watcher.watch(&index, RecursiveMode::NonRecursive);
        }

        // COMMIT_EDITMSG - commit message edits
        let commit_editmsg = git_dir.join("COMMIT_EDITMSG");
        if commit_editmsg.exists() {
            let _ = watcher.watch(&commit_editmsg, RecursiveMode::NonRecursive);
        }

        // refs/heads - local branch changes
        let heads = git_dir.join("refs").join("heads");
        if heads.exists() {
            let _ = watcher.watch(&heads, RecursiveMode::Recursive);
        }

        // refs/remotes - remote branch changes (small)
        let remotes = git_dir.join("refs").join("remotes");
        if remotes.exists() {
            let _ = watcher.watch(&remotes, RecursiveMode::Recursive);
        }
    }

    // Directories that should never be watched recursively as "sources"
    let skip = [
        "node_modules",
        ".next",
        "dist",
        "build",
        "target",
        ".turbo",
        ".cache",
        "__pycache__",
        ".venv",
        "venv",
        "vendor",
        "Pods",
        "coverage",
        ".git", // entire .git handled separately above
        ".idea",
    ];

    // Watch recursively up to depth 4, skipping heavy directories
    log::debug!("Setting up source file watching...");
    watch_recursive_limited(&mut watcher, &real_repo_path, 0, 4, &skip);
    log::debug!("Source file watching setup complete");

    *handle = Some(watcher);
    IS_RUNNING.store(true, Ordering::Relaxed);
    log::info!("File watcher started for {}", repo_path_str);
    Ok(())
}

pub fn stop_watching() {
    IS_RUNNING.store(false, Ordering::Relaxed);
    if let Ok(mut handle) = WATCHER.lock() {
        *handle = None;
    }
}
