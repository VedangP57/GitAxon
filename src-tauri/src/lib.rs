use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tauri_plugin_store::StoreExt;

#[cfg(target_os = "macos")]
use libc;

use gitaxon::cache::Cache;

mod github;

// ─── Batch response types ──────────────────────────────────────────

#[derive(serde::Serialize)]
struct RepoState {
    commits: String,
    branches: String,
    tags: String,
    status: String,
}

#[derive(serde::Serialize)]
struct GraphState {
    commits: String,
    branches: String,
    tags: String,
}

struct StatusCache {
    repo_path: String,
    result: String,
    fetched_at: Instant,
}

static STATUS_CACHE: Mutex<Option<StatusCache>> = Mutex::new(None);

fn invalidate_status_cache() {
    if let Ok(mut cache) = STATUS_CACHE.lock() {
        *cache = None;
    }
}

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
async fn start_file_watch(repo_path: String, app: AppHandle) -> Result<(), String> {
    let repo_path_str = repo_path.clone();

    // Channel for worktree file changes → update status + emit worktree-changed
    let (wt_tx, wt_rx) = std::sync::mpsc::channel::<()>();
    let wt_app = app.clone();
    let wt_repo = repo_path_str.clone();
    std::thread::spawn(move || {
        while wt_rx.recv().is_ok() {
            while wt_rx.try_recv().is_ok() {}
            match gitaxon::watcher::update_status(&wt_repo) {
                Ok(patch) => {
                    if patch.added.is_empty() && patch.removed.is_empty() && patch.changed.is_empty() {
                        continue;
                    }
                    if let Ok(payload) = serde_json::to_string(&patch) {
                        let _ = wt_app.emit("worktree-changed", payload);
                    }
                }
                Err(_) => {}
            }
        }
    });

    // Channel for git-state changes → emit git-state-changed
    let (gs_tx, gs_rx) = std::sync::mpsc::channel::<()>();
    let gs_app = app.clone();
    std::thread::spawn(move || {
        while gs_rx.recv().is_ok() {
            while gs_rx.try_recv().is_ok() {}
            let _ = gs_app.emit("git-state-changed", ());
        }
    });

    gitaxon::watcher::start_watching(
        &repo_path,
        move || { let _ = wt_tx.send(()); },
        move || { let _ = gs_tx.send(()); },
    )?;

    Ok(())
}

#[tauri::command]
async fn stop_file_watch(repo_path: Option<String>) -> Result<(), String> {
    if let Some(path) = repo_path {
        gitaxon::watcher::stop_watching(&path);
    }
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
    let repo = gitaxon::repo_pool::open_repo(&repo_path).map_err(|e| e.to_string())?;
    let config = repo.config().map_err(|e| e.to_string())?;
    config.get_string(&key).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_status(repo_path: String) -> Result<String, String> {
    // Fast path: return cached result if very recent for same repo
    {
        let cache = STATUS_CACHE.lock().map_err(|e| e.to_string())?;
        if let Some(ref c) = *cache {
            if c.repo_path == repo_path && c.fetched_at.elapsed() < Duration::from_millis(500) {
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

    if let Ok(mut cache) = STATUS_CACHE.lock() {
        *cache = Some(StatusCache {
            repo_path,
            result: result.clone(),
            fetched_at: Instant::now(),
        });
    }

    Ok(result)
}

#[tauri::command]
async fn get_bisect_state(repo_path: String) -> Result<String, String> {
    let result = tokio::task::spawn_blocking(move || {
        gitaxon::bisect::get_bisect_state(&repo_path)
    }).await.map_err(|e| e.to_string())??;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_bisect(repo_path: String, bad_hash: String, good_hash: String) -> Result<String, String> {
    let result = tokio::task::spawn_blocking(move || {
        gitaxon::bisect::start_bisect(&repo_path, &bad_hash, &good_hash)
    }).await.map_err(|e| e.to_string())??;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn bisect_good(repo_path: String) -> Result<String, String> {
    let result = tokio::task::spawn_blocking(move || {
        gitaxon::bisect::bisect_good(&repo_path)
    }).await.map_err(|e| e.to_string())??;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn bisect_bad(repo_path: String) -> Result<String, String> {
    let result = tokio::task::spawn_blocking(move || {
        gitaxon::bisect::bisect_bad(&repo_path)
    }).await.map_err(|e| e.to_string())??;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn bisect_skip(repo_path: String) -> Result<String, String> {
    let result = tokio::task::spawn_blocking(move || {
        gitaxon::bisect::bisect_skip(&repo_path)
    }).await.map_err(|e| e.to_string())??;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn bisect_reset(repo_path: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::bisect::bisect_reset(&repo_path)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn stage_hunk(repo_path: String, patch_text: String, app: AppHandle) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::staging::stage_hunk(&repo_path, &patch_text)
    })
    .await
    .map_err(|e| e.to_string())??;
    invalidate_status_cache();
    let _ = app.emit("worktree-changed", "");
    Ok(())
}

#[tauri::command]
async fn unstage_hunk(repo_path: String, patch_text: String, app: AppHandle) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::staging::unstage_hunk(&repo_path, &patch_text)
    })
    .await
    .map_err(|e| e.to_string())??;
    invalidate_status_cache();
    let _ = app.emit("worktree-changed", "");
    Ok(())
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

    invalidate_status_cache();

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

    invalidate_status_cache();

    let _ = app.emit("git-state-changed", repo_path.clone());

    Ok(())
}

#[tauri::command]
async fn unstage_all(repo_path: String, app: AppHandle) -> Result<(), String> {
    gitaxon::staging::unstage_all(&repo_path)
        .await
        .map_err(|e| e.to_string())?;

    invalidate_status_cache();

    let _ = app.emit("git-state-changed", repo_path.clone());

    Ok(())
}

#[tauri::command]
async fn stage_all(repo_path: String, app: AppHandle) -> Result<(), String> {
    gitaxon::staging::stage_all(&repo_path)
        .await
        .map_err(|e| e.to_string())?;

    invalidate_status_cache();

    let _ = app.emit("git-state-changed", repo_path.clone());

    Ok(())
}

#[tauri::command]
async fn create_commit(
    repo_path: String,
    message: String,
    author_name: String,
    author_email: String,
    amend: Option<bool>,
    app: AppHandle,
) -> Result<String, String> {
    let result = if amend.unwrap_or(false) {
        let rp = repo_path.clone();
        tokio::task::spawn_blocking(move || {
            gitaxon::staging::amend_commit(&rp, &message, &author_name, &author_email)
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?
    } else {
        gitaxon::staging::create_commit(&repo_path, &message, &author_name, &author_email)
            .await
            .map_err(|e| e.to_string())?
    };

    invalidate_status_cache();
    let _ = app.emit("git-state-changed", repo_path.clone());

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
async fn get_rebase_state(repo_path: String) -> Result<String, String> {
    let result = tokio::task::spawn_blocking(move || {
        gitaxon::rebase::get_rebase_state(&repo_path)
    })
    .await
    .map_err(|e| e.to_string())??;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_rebase_todo_for_range(repo_path: String, onto: String) -> Result<String, String> {
    let result = tokio::task::spawn_blocking(move || {
        gitaxon::rebase::get_rebase_todo_for_range(&repo_path, &onto)
    })
    .await
    .map_err(|e| e.to_string())??;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_interactive_rebase(repo_path: String, onto: String, todo: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::rebase::start_interactive_rebase(&repo_path, &onto, &todo)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn continue_rebase(repo_path: String, new_message: Option<String>) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::rebase::continue_rebase(&repo_path, new_message.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn abort_rebase(repo_path: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::rebase::abort_rebase(&repo_path)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn skip_rebase(repo_path: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::rebase::skip_rebase(&repo_path)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn detect_operation_state(repo_path: String) -> Result<String, String> {
    let result = tokio::task::spawn_blocking(move || {
        gitaxon::conflicts::detect_operation_state(&repo_path)
    })
    .await
    .map_err(|e| e.to_string())??;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_conflict_file(repo_path: String, file_path: String) -> Result<String, String> {
    let result = tokio::task::spawn_blocking(move || {
        gitaxon::conflicts::get_conflict_file(&repo_path, &file_path)
    })
    .await
    .map_err(|e| e.to_string())??;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn resolve_conflict(repo_path: String, file_path: String, resolved_content: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::conflicts::resolve_conflict(&repo_path, &file_path, &resolved_content)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn continue_operation(repo_path: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::conflicts::continue_operation(&repo_path)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn abort_operation(repo_path: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::conflicts::abort_operation(&repo_path)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn search_commits(
    repo_path: String,
    query: Option<String>,
    author: Option<String>,
    since: Option<String>,
    until: Option<String>,
    path: Option<String>,
    limit: usize,
) -> Result<String, String> {
    let result = tokio::task::spawn_blocking(move || {
        let commits = gitaxon::graph::search_commits(
            &repo_path,
            query.as_deref(),
            author.as_deref(),
            since.as_deref(),
            until.as_deref(),
            path.as_deref(),
            limit,
        )?;
        let laned = gitaxon::assign_lanes(commits);
        serde_json::to_string(&laned).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(result)
}

#[tauri::command]
async fn list_worktrees(repo_path: String) -> Result<String, String> {
    let result = tokio::task::spawn_blocking(move || {
        gitaxon::worktree::list_worktrees(&repo_path)
    })
    .await
    .map_err(|e| e.to_string())??;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn add_worktree(repo_path: String, path: String, branch: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::worktree::add_worktree(&repo_path, &path, &branch)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn remove_worktree(repo_path: String, path: String, force: bool) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::worktree::remove_worktree(&repo_path, &path, force)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_remotes(repo_path: String) -> Result<String, String> {
    gitaxon::remotes::list_remotes_json(&repo_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn add_remote(repo_path: String, name: String, url: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::remotes::add_remote(&repo_path, &name, &url).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn remove_remote(repo_path: String, name: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::remotes::remove_remote(&repo_path, &name).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn rename_remote(repo_path: String, old_name: String, new_name: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::remotes::rename_remote(&repo_path, &old_name, &new_name).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn set_remote_url(repo_path: String, name: String, url: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::remotes::set_remote_url(&repo_path, &name, &url).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
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
async fn checkout_branch(repo_path: String, name: String, app: AppHandle) -> Result<(), String> {
    gitaxon::branches::checkout_branch(&repo_path, &name)
        .await
        .map_err(|e| e.to_string())?;

    invalidate_status_cache();
    let _ = app.emit("git-state-changed", repo_path.clone());

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
async fn file_history(repo_path: String, file_path: String, limit: usize) -> Result<String, String> {
    let result = tokio::task::spawn_blocking(move || {
        gitaxon::history::file_history(&repo_path, &file_path, limit)
    })
    .await
    .map_err(|e| e.to_string())??;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn file_diff_at_commit(repo_path: String, file_path: String, commit_hash: String) -> Result<String, String> {
    let result = tokio::task::spawn_blocking(move || {
        gitaxon::history::file_diff_at_commit(&repo_path, &file_path, &commit_hash)
    })
    .await
    .map_err(|e| e.to_string())??;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn stash_show(repo_path: String, index: usize) -> Result<String, String> {
    let result = tokio::task::spawn_blocking(move || {
        gitaxon::stash::stash_show(&repo_path, index)
    })
    .await
    .map_err(|e| e.to_string())??;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_tags(repo_path: String) -> Result<String, String> {
    gitaxon::tags::list_tags_json(&repo_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_tag(
    repo_path: String,
    name: String,
    target_hash: String,
    message: String,
) -> Result<(), String> {
    gitaxon::tags::create_tag(&repo_path, &name, &target_hash, &message)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_tag(repo_path: String, name: String) -> Result<(), String> {
    gitaxon::tags::delete_tag(&repo_path, &name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn push_tag(
    repo_path: String,
    remote_name: String,
    tag_name: String,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::tags::push_tag(&repo_path, &remote_name, &tag_name)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn delete_remote_tag(
    repo_path: String,
    remote_name: String,
    tag_name: String,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::tags::delete_remote_tag(&repo_path, &remote_name, &tag_name)
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
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
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

// ─── GitHub/GitLab Integration ──────────────────────────────────────

fn get_repo_coords(repo_path: &str) -> Result<github::RepoCoords, String> {
    let repo = gitaxon::repo_pool::open_repo(repo_path).map_err(|e| e.to_string())?;
    let remote = repo
        .find_remote("origin")
        .map_err(|e| format!("No origin remote: {}", e))?;
    let url = remote.url().unwrap_or("");
    let coords = github::parse_remote_url(url);
    if coords.platform == github::Platform::Unknown {
        return Err("Not a GitHub or GitLab repository".to_string());
    }
    Ok(coords)
}

fn get_token(_app: &AppHandle, platform: &str) -> String {
    keyring::Entry::new("gitaxon", platform)
        .ok()
        .and_then(|e| e.get_password().ok())
        .unwrap_or_default()
}

#[tauri::command]
async fn detect_platform(repo_path: String) -> Result<String, String> {
    let coords = get_repo_coords(&repo_path)?;
    serde_json::to_string(&coords).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_prs(repo_path: String, app: AppHandle) -> Result<String, String> {
    let coords = get_repo_coords(&repo_path)?;
    let token = get_token(&app, &format!("{:?}", coords.platform).to_lowercase());
    let prs = github::list_github_prs(&coords, &token).await?;
    serde_json::to_string(&prs).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_issues(repo_path: String, app: AppHandle) -> Result<String, String> {
    let coords = get_repo_coords(&repo_path)?;
    let token = get_token(&app, &format!("{:?}", coords.platform).to_lowercase());
    let issues = github::list_github_issues(&coords, &token).await?;
    serde_json::to_string(&issues).map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_pr(
    repo_path: String,
    title: String,
    body: String,
    head: String,
    base: String,
    app: AppHandle,
) -> Result<String, String> {
    let coords = get_repo_coords(&repo_path)?;
    let token = get_token(&app, &format!("{:?}", coords.platform).to_lowercase());
    if token.is_empty() {
        return Err("No API token configured. Add your token in Settings.".to_string());
    }
    let pr = github::create_github_pr(&coords, &token, &title, &body, &head, &base).await?;
    serde_json::to_string(&pr).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_pr_comments(repo_path: String, pr_number: u64, app: AppHandle) -> Result<String, String> {
    let coords = get_repo_coords(&repo_path)?;
    let token = get_token(&app, &format!("{:?}", coords.platform).to_lowercase());
    let comments = github::get_pr_comments(&coords, &token, pr_number).await?;
    serde_json::to_string(&comments).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_pr_files(repo_path: String, pr_number: u64, app: AppHandle) -> Result<String, String> {
    let coords = get_repo_coords(&repo_path)?;
    let token = get_token(&app, &format!("{:?}", coords.platform).to_lowercase());
    let files = github::get_pr_files(&coords, &token, pr_number).await?;
    serde_json::to_string(&files).map_err(|e| e.to_string())
}

#[tauri::command]
async fn submit_pr_review(repo_path: String, pr_number: u64, body: String, event: String, app: AppHandle) -> Result<String, String> {
    let coords = get_repo_coords(&repo_path)?;
    let token = get_token(&app, &format!("{:?}", coords.platform).to_lowercase());
    github::submit_pr_review(&coords, &token, pr_number, &body, &event).await
}

#[tauri::command]
async fn set_api_token(platform: String, token: String, app: AppHandle) -> Result<(), String> {
    // Migrate any existing plaintext token from the old store
    if let Ok(store) = app.store("tokens.json") {
        let _ = store.delete(&platform);
        let _ = store.save();
    }
    let entry = keyring::Entry::new("gitaxon", &platform).map_err(|e| e.to_string())?;
    entry.set_password(&token).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn get_api_token(platform: String, _app: AppHandle) -> Result<String, String> {
    Ok(keyring::Entry::new("gitaxon", &platform)
        .ok()
        .and_then(|e| e.get_password().ok())
        .unwrap_or_default())
}

#[tauri::command]
async fn open_terminal_at(repo_path: String) -> Result<(), String> {
    // Validate the path exists and is a directory
    let path = std::path::Path::new(&repo_path);
    if !path.is_dir() {
        return Err(format!("Not a valid directory: {}", repo_path));
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        // Use `open -a Terminal` with the directory path as an argument,
        // avoiding shell/AppleScript injection entirely.
        let status = Command::new("open")
            .args(["-a", "Terminal", &repo_path])
            .status();
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
        let repo = gitaxon::repo_pool::open_repo(&repo_path).map_err(|e| e.to_string())?;
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
        invalidate_status_cache();
    }
    result
}

#[tauri::command]
async fn discard_all_changes(repo_path: String) -> Result<(), String> {
    let result = tokio::task::spawn_blocking(move || {
        let repo = gitaxon::repo_pool::open_repo(&repo_path).map_err(|e| e.to_string())?;
        gitaxon::staging::discard_all(&repo).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?;
    if result.is_ok() {
        invalidate_status_cache();
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

// ─── Batch commands: reduce IPC round-trips ────────────────────────

/// Single IPC call to load all repo data on open.
/// Replaces 4 separate calls: getCommits + getBranches + getTags + getStatus
/// The `limit` parameter controls how many commits to fetch (default 500).
/// Sub-fetch failures return empty defaults rather than failing the whole response.
#[tauri::command]
async fn get_repo_state(repo_path: String, limit: Option<usize>) -> Result<RepoState, String> {
    let rp = repo_path.clone();
    let commit_limit = limit.unwrap_or(500);

    let (commits_res, branches_res, tags_res, status_res) = tokio::join!(
        gitaxon::graph::get_laned_commits_json(&rp, commit_limit, 0),
        gitaxon::branches::list_branches_json(&rp),
        gitaxon::tags::list_tags_json(&rp),
        async {
            let path = rp.clone();
            let entries = tokio::task::spawn_blocking(move || {
                gitaxon::watcher::get_full_status_and_sync_snapshot(&path)
            })
            .await
            .map_err(|e| gitaxon::GitfastError::GitOperationFailed(e.to_string()))?
            .map_err(|e| gitaxon::GitfastError::GitOperationFailed(e))?;
            serde_json::to_string(&entries)
                .map_err(|e| gitaxon::GitfastError::SerializationError(e.to_string()))
        }
    );

    // Partial failures return empty defaults — a corrupted tag won't prevent the repo from opening
    let commits = commits_res.unwrap_or_else(|_| "[]".to_string());
    let branches = branches_res.unwrap_or_else(|_| "[]".to_string());
    let tags = tags_res.unwrap_or_else(|_| "[]".to_string());
    let status = status_res.unwrap_or_else(|_| "[]".to_string());

    if let Ok(mut cache) = STATUS_CACHE.lock() {
        *cache = Some(StatusCache {
            repo_path,
            result: status.clone(),
            fetched_at: Instant::now(),
        });
    }

    Ok(RepoState {
        commits,
        branches,
        tags,
        status,
    })
}

/// Single IPC call to reload graph state after git events.
/// Replaces 3 separate calls: getCommits + getBranches + getTags
/// The `limit` parameter controls how many commits to fetch (default 500).
#[tauri::command]
async fn get_graph_state(repo_path: String, limit: Option<usize>) -> Result<GraphState, String> {
    let rp = repo_path.clone();
    let commit_limit = limit.unwrap_or(500);

    let (commits_res, branches_res, tags_res) = tokio::join!(
        gitaxon::graph::get_laned_commits_json(&rp, commit_limit, 0),
        gitaxon::branches::list_branches_json(&rp),
        gitaxon::tags::list_tags_json(&rp),
    );

    Ok(GraphState {
        commits: commits_res.unwrap_or_else(|_| "[]".to_string()),
        branches: branches_res.unwrap_or_else(|_| "[]".to_string()),
        tags: tags_res.unwrap_or_else(|_| "[]".to_string()),
    })
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
        log::info!("[System] fd limit set to {}", rlim.rlim_cur);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
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
                gitaxon::watcher::stop_all_watchers();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_repo_state,
            get_graph_state,
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
            stage_hunk,
            unstage_hunk,
            get_bisect_state,
            start_bisect,
            bisect_good,
            bisect_bad,
            bisect_skip,
            bisect_reset,
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
            stash_show,
            file_history,
            file_diff_at_commit,
            search_commits,
            get_rebase_state,
            get_rebase_todo_for_range,
            start_interactive_rebase,
            continue_rebase,
            abort_rebase,
            skip_rebase,
            detect_operation_state,
            get_conflict_file,
            resolve_conflict,
            continue_operation,
            abort_operation,
            list_worktrees,
            add_worktree,
            remove_worktree,
            list_remotes,
            add_remote,
            remove_remote,
            rename_remote,
            set_remote_url,
            detect_platform,
            list_prs,
            list_issues,
            create_pr,
            get_pr_comments,
            get_pr_files,
            submit_pr_review,
            set_api_token,
            get_api_token,
            list_tags,
            create_tag,
            delete_tag,
            push_tag,
            delete_remote_tag,
            open_terminal_at,
            discard_file,
            discard_all_changes,
            start_file_watch,
            stop_file_watch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
