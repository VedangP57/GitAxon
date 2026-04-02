use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

#[cfg(target_os = "macos")]
use libc;

use gitaxon::cache::Cache;

struct StatusCache {
    repo_path: String,
    result: String,
    fetched_at: Instant,
}

static STATUS_CACHE: Mutex<Option<StatusCache>> = Mutex::new(None);

#[tauri::command]
async fn get_full_status(
    repo_path: String,
) -> Result<Vec<gitaxon::watcher::FileStatus>, String> {
    // Ensure repo status is initialized
    gitaxon::watcher::init_status(&repo_path)?;

    let state = gitaxon::watcher::REPO_STATUS
        .read()
        .map_err(|e| e.to_string())?;

    Ok(state
        .get(&repo_path)
        .cloned()
        .unwrap_or_default())
}

#[tauri::command]
async fn start_file_watch(
    repo_path: String,
    app: AppHandle,
) -> Result<(), String> {
    println!("[Tauri] start_file_watch called for: {}", &repo_path);

    // TEST: emit immediately to verify channel works
    let _ = app.emit("worktree-changed", "TEST_EVENT");
    println!("[Tauri] Test event emitted");

    let app_handle = app.clone();
    let repo_path_str = repo_path.clone();

    gitaxon::watcher::start_watching(&repo_path, move || {
        let app = app_handle.clone();
        let repo = repo_path_str.clone();

        // Compute status diff on a background thread
        std::thread::spawn(move || {
            match gitaxon::watcher::update_status(&repo) {
                Ok(patch) => {
                    if patch.added.is_empty()
                        && patch.removed.is_empty()
                        && patch.changed.is_empty()
                    {
                        // No actual status change: no event
                        return;
                    }

                    match serde_json::to_string(&patch) {
                        Ok(payload) => {
                            let _ = app.emit("worktree-changed", payload);
                            println!(
                                "[Watcher] Patch: +{} -{} ~{}",
                                patch.added.len(),
                                patch.removed.len(),
                                patch.changed.len()
                            );
                        }
                        Err(e) => {
                            println!("[Watcher] Failed to serialize patch: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("[Watcher] Error computing status patch: {}", e);
                }
            }
        });
    })?;

    println!("[Tauri] start_file_watch complete");
    Ok(())
}

#[tauri::command]
async fn stop_file_watch() -> Result<(), String> {
    gitaxon::watcher::stop_watching();
    Ok(())
}

#[tauri::command]
async fn get_commits(
    repo_path: String,
    limit: usize,
    offset: usize,
) -> Result<String, String> {
    gitaxon::graph::get_laned_commits_json(&repo_path, limit, offset)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_branches(repo_path: String) -> Result<String, String> {
    gitaxon::branches::list_branches_json(&repo_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_git_config(repo_path: String, key: String) -> Result<String, String> {
    let repo = git2::Repository::open(&repo_path).map_err(|e| e.to_string())?;
    let config = repo.config().map_err(|e| e.to_string())?;
    config.get_string(&key).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_status(repo_path: String) -> Result<String, String> {
    // Fast path: return cached result if very recent for same repo
    {
        let cache = STATUS_CACHE.lock().unwrap();
        if let Some(ref c) = *cache {
            if c.repo_path == repo_path && c.fetched_at.elapsed() < Duration::from_millis(150) {
                return Ok(c.result.clone());
            }
        }
    }

    // Always run fresh git status - NEVER compare against PREV_STATUS.
    // PREV_STATUS is only used by the watcher for diff detection.
    let entries = tokio::task::spawn_blocking({
        let path = repo_path.clone();
        move || gitaxon::watcher::get_full_status_and_sync_snapshot(&path)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    let result =
        serde_json::to_string(&entries).map_err(|e| e.to_string())?;

    {
        let mut cache = STATUS_CACHE.lock().unwrap();
        *cache = Some(StatusCache {
            repo_path,
            result: result.clone(),
            fetched_at: Instant::now(),
        });
    }

    Ok(result)
}

#[tauri::command]
async fn stage_file(
    repo_path: String,
    file_path: String,
    app: AppHandle,
) -> Result<(), String> {
    gitaxon::staging::stage_file(&repo_path, &file_path)
        .await
        .map_err(|e| e.to_string())?;

    // Invalidate status cache after staging operations
    {
        let mut cache = STATUS_CACHE.lock().unwrap();
        *cache = None;
    }

    let _ = app.emit("git-state-changed", repo_path.clone());

    Ok(())
}

#[tauri::command]
async fn unstage_file(
    repo_path: String,
    file_path: String,
    app: AppHandle,
) -> Result<(), String> {
    gitaxon::staging::unstage_file(&repo_path, &file_path)
        .await
        .map_err(|e| e.to_string())?;

    {
        let mut cache = STATUS_CACHE.lock().unwrap();
        *cache = None;
    }

    let _ = app.emit("git-state-changed", repo_path.clone());

    Ok(())
}

#[tauri::command]
async fn unstage_all(repo_path: String, app: AppHandle) -> Result<(), String> {
    gitaxon::staging::unstage_all(&repo_path)
        .await
        .map_err(|e| e.to_string())?;

    {
        let mut cache = STATUS_CACHE.lock().unwrap();
        *cache = None;
    }

    let _ = app.emit("git-state-changed", repo_path.clone());

    Ok(())
}

#[tauri::command]
async fn stage_all(repo_path: String, app: AppHandle) -> Result<(), String> {
    gitaxon::staging::stage_all(&repo_path)
        .await
        .map_err(|e| e.to_string())?;

    {
        let mut cache = STATUS_CACHE.lock().unwrap();
        *cache = None;
    }

    let _ = app.emit("git-state-changed", repo_path.clone());

    Ok(())
}

#[tauri::command]
async fn create_commit(
    repo_path: String,
    message: String,
    author_name: String,
    author_email: String,
) -> Result<String, String> {
    let result = gitaxon::staging::create_commit(&repo_path, &message, &author_name, &author_email)
        .await
        .map_err(|e| e.to_string())?;

    {
        let mut cache = STATUS_CACHE.lock().unwrap();
        *cache = None;
    }

    Ok(result)
}

#[tauri::command]
async fn cherry_pick(
    repo_path: String,
    commit_hash: String,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::staging::cherry_pick(&repo_path, &commit_hash)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn revert_commit(
    repo_path: String,
    commit_hash: String,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::staging::revert_commit(&repo_path, &commit_hash)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn reset_to_commit(
    repo_path: String,
    commit_hash: String,
    mode: String,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::staging::reset_to_commit(&repo_path, &commit_hash, &mode)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_diff_commit(
    repo_path: String,
    commit_hash: String,
) -> Result<String, String> {
    gitaxon::diff::diff_commit_json(&repo_path, &commit_hash)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_diff_working_tree(repo_path: String) -> Result<String, String> {
    gitaxon::diff::diff_working_tree_json(&repo_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_diff_staged(repo_path: String) -> Result<String, String> {
    gitaxon::diff::diff_staged_json(&repo_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn read_diff_file_content(
    repo_path: String,
    file_path: String,
    mode: String,
    commit_hash: Option<String>,
    file_status: Option<String>,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::diff::read_diff_file_content(
            &repo_path,
            &file_path,
            &mode,
            commit_hash.as_deref(),
            file_status.as_deref(),
        )
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn git_blame(
    repo_path: String,
    file_path: String,
    commit_hash: Option<String>,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let blame = gitaxon::diff::git_blame(&repo_path, &file_path, commit_hash.as_deref())?;
        serde_json::to_string(&blame).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn pull(
    repo_path: String,
    remote_name: String,
    branch_name: String,
) -> Result<String, String> {
    gitaxon::remotes::pull(&repo_path, &remote_name, &branch_name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn push(
    repo_path: String,
    remote_name: String,
    branch_name: String,
    force: bool,
) -> Result<String, String> {
    let result = gitaxon::remotes::push(&repo_path, &remote_name, &branch_name, force)
        .await
        .map_err(|e| e.to_string())?;
    if result.success {
        Ok(result.message)
    } else {
        Err(result.message)
    }
}

#[tauri::command]
async fn fetch_remote(
    repo_path: String,
    remote_name: String,
) -> Result<String, String> {
    let result = gitaxon::remotes::fetch(&repo_path, &remote_name)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_recent_repositories(limit: usize) -> Result<String, String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let cache_dir = format!("{}/.gitfast", home);
    std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
    let db_path = format!("{}/cache.db", cache_dir);

    let cache = Cache::new(&db_path).map_err(|e| e.to_string())?;
    let repos = cache.get_recent_repositories(limit).map_err(|e| e.to_string())?;
    serde_json::to_string(&repos).map_err(|e| e.to_string())
}

#[tauri::command]
async fn checkout_branch(repo_path: String, name: String) -> Result<(), String> {
    gitaxon::branches::checkout_branch(&repo_path, &name)
        .await
        .map_err(|e| e.to_string())?;

    {
        let mut cache = STATUS_CACHE.lock().unwrap();
        *cache = None;
    }

    Ok(())
}

#[tauri::command]
async fn delete_branch(repo_path: String, name: String, force: bool) -> Result<(), String> {
    gitaxon::branches::delete_branch(&repo_path, &name, force)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn rename_branch(repo_path: String, old_name: String, new_name: String) -> Result<(), String> {
    gitaxon::branches::rename_branch(&repo_path, &old_name, &new_name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_branch(repo_path: String, name: String, from_ref: String) -> Result<(), String> {
    gitaxon::branches::create_branch(&repo_path, &name, &from_ref)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn merge_branch(repo_path: String, branch_name: String) -> Result<(), String> {
    gitaxon::branches::merge_branch(&repo_path, &branch_name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_stashes(repo_path: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let entries = gitaxon::stash::list_stashes(&repo_path)?;
        serde_json::to_string(&entries).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn stash_push(repo_path: String, message: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::stash::stash_push(&repo_path, &message)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn stash_pop(repo_path: String, index: usize) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::stash::stash_pop(&repo_path, index)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn stash_apply(repo_path: String, index: usize) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::stash::stash_apply(&repo_path, index)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn stash_drop(repo_path: String, index: usize) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::stash::stash_drop(&repo_path, index)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn stash_branch(
    repo_path: String,
    index: usize,
    branch_name: String,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::stash::stash_branch(&repo_path, index, &branch_name)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_repo_identity(repo_path: String) -> Result<String, String> {
    let path = repo_path.clone();
    let identity = tokio::task::spawn_blocking(move || {
        gitaxon::identity::get_repo_identity(&path)
    })
    .await
    .map_err(|e| e.to_string())??;
    serde_json::to_string(&identity).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_ssh_profiles() -> Result<String, String> {
    let profiles = gitaxon::identity::get_ssh_profiles();
    serde_json::to_string(&profiles).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_ssh_username(host_alias: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::identity::get_github_username_cached(&host_alias)
            .ok_or_else(|| "Could not get username".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn switch_repo_identity(
    repo_path: String,
    ssh_host_alias: String,
    name: String,
    email: String,
) -> Result<(), String> {
    gitaxon::identity::switch_repo_identity(
        &repo_path,
        &ssh_host_alias,
        &name,
        &email,
    )
}

#[tauri::command]
async fn open_terminal_at(repo_path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let path_escaped = repo_path.replace('"', "\\\"");
        let script = format!(
            "tell application \"Terminal\" to do script \"cd \\\"{}\\\"\"",
            path_escaped
        );
        let status = Command::new("osascript").args(["-e", &script]).status();
        status.map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = repo_path;
        return Err("Opening terminal is only supported on macOS".to_string());
    }
    Ok(())
}

#[tauri::command]
async fn discard_file(
    repo_path: String,
    file_path: String,
) -> Result<(), String> {
    let result = tokio::task::spawn_blocking(move || {
        let repo = git2::Repository::open(&repo_path).map_err(|e| e.to_string())?;
        let status = repo
            .status_file(std::path::Path::new(&file_path))
            .map_err(|e| e.to_string())?;
        if status.contains(git2::Status::WT_NEW) {
            gitaxon::staging::delete_untracked(&repo, &file_path)
                .map_err(|e| e.to_string())
        } else {
            gitaxon::staging::discard_file(&repo, &file_path)
                .map_err(|e| e.to_string())
        }
    })
    .await
    .map_err(|e| e.to_string())?;
    if result.is_ok() {
        let mut cache = STATUS_CACHE.lock().unwrap();
        *cache = None;
    }
    result
}

#[tauri::command]
async fn discard_all_changes(repo_path: String) -> Result<(), String> {
    let result = tokio::task::spawn_blocking(move || {
        let repo = git2::Repository::open(&repo_path).map_err(|e| e.to_string())?;
        gitaxon::staging::discard_all(&repo).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?;
    if result.is_ok() {
        let mut cache = STATUS_CACHE.lock().unwrap();
        *cache = None;
    }
    result
}

#[tauri::command]
async fn open_repository(repo_path: String) -> Result<String, String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let cache_dir = format!("{}/.gitfast", home);
    std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
    let db_path = format!("{}/cache.db", cache_dir);

    let cache = Cache::new(&db_path).map_err(|e| e.to_string())?;
    let repo_name = Path::new(&repo_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repository")
        .to_string();
    cache
        .add_repository(&repo_path, &repo_name)
        .map_err(|e| e.to_string())?;

    // Pre-compute status into memory for this repo (for get_full_status).
    // PREV_STATUS is synced when get_status runs - never in open_repository.

    Ok(repo_path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Increase file descriptor limit for file watching on macOS
    #[cfg(target_os = "macos")]
    unsafe {
        let mut rlim = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        libc::getrlimit(libc::RLIMIT_NOFILE, &mut rlim);
        rlim.rlim_cur = 65536.min(rlim.rlim_max);
        libc::setrlimit(libc::RLIMIT_NOFILE, &rlim);
        println!("[System] fd limit set to {}", rlim.rlim_cur);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                gitaxon::watcher::stop_watching();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_full_status,
            get_repo_identity,
            get_ssh_profiles,
            get_ssh_username,
            switch_repo_identity,
            get_recent_repositories,
            get_commits,
            get_branches,
            get_git_config,
            get_status,
            checkout_branch,
            delete_branch,
            rename_branch,
            create_branch,
            merge_branch,
            stage_file,
            unstage_file,
            stage_all,
            unstage_all,
            create_commit,
            cherry_pick,
            revert_commit,
            reset_to_commit,
            get_diff_commit,
            get_diff_working_tree,
            get_diff_staged,
            read_diff_file_content,
            git_blame,
            pull,
            push,
            fetch_remote,
            open_repository,
            list_stashes,
            stash_push,
            stash_pop,
            stash_apply,
            stash_drop,
            stash_branch,
            open_terminal_at,
            discard_file,
            discard_all_changes,
            start_file_watch,
            stop_file_watch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
