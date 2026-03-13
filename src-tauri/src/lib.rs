use std::path::Path;

use gitfast_core::cache::Cache;

#[tauri::command]
async fn get_commits(
    repo_path: String,
    limit: usize,
    offset: usize,
) -> Result<String, String> {
    gitfast_core::graph::get_laned_commits_json(&repo_path, limit, offset)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_branches(repo_path: String) -> Result<String, String> {
    gitfast_core::branches::list_branches_json(&repo_path)
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
    gitfast_core::staging::get_status_json(&repo_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn stage_file(repo_path: String, file_path: String) -> Result<(), String> {
    gitfast_core::staging::stage_file(&repo_path, &file_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn unstage_file(repo_path: String, file_path: String) -> Result<(), String> {
    gitfast_core::staging::unstage_file(&repo_path, &file_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn unstage_all(repo_path: String) -> Result<(), String> {
    gitfast_core::staging::unstage_all(&repo_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn stage_all(repo_path: String) -> Result<(), String> {
    gitfast_core::staging::stage_all(&repo_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_commit(
    repo_path: String,
    message: String,
    author_name: String,
    author_email: String,
) -> Result<String, String> {
    gitfast_core::staging::create_commit(&repo_path, &message, &author_name, &author_email)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_diff_commit(
    repo_path: String,
    commit_hash: String,
) -> Result<String, String> {
    gitfast_core::diff::diff_commit_json(&repo_path, &commit_hash)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_diff_working_tree(repo_path: String) -> Result<String, String> {
    gitfast_core::diff::diff_working_tree_json(&repo_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_diff_staged(repo_path: String) -> Result<String, String> {
    gitfast_core::diff::diff_staged_json(&repo_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn pull(
    repo_path: String,
    remote_name: String,
    branch_name: String,
) -> Result<String, String> {
    gitfast_core::remotes::pull(&repo_path, &remote_name, &branch_name)
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
    let result = gitfast_core::remotes::push(&repo_path, &remote_name, &branch_name, force)
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
    let result = gitfast_core::remotes::fetch(&repo_path, &remote_name)
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
    gitfast_core::branches::checkout_branch(&repo_path, &name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_branch(repo_path: String, name: String, force: bool) -> Result<(), String> {
    gitfast_core::branches::delete_branch(&repo_path, &name, force)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn rename_branch(repo_path: String, old_name: String, new_name: String) -> Result<(), String> {
    gitfast_core::branches::rename_branch(&repo_path, &old_name, &new_name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_branch(repo_path: String, name: String, from_ref: String) -> Result<(), String> {
    gitfast_core::branches::create_branch(&repo_path, &name, &from_ref)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn merge_branch(repo_path: String, branch_name: String) -> Result<(), String> {
    gitfast_core::branches::merge_branch(&repo_path, &branch_name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn stash_push(repo_path: String, message: Option<String>) -> Result<(), String> {
    gitfast_core::stash::stash_push(&repo_path, message.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn stash_pop(repo_path: String) -> Result<(), String> {
    gitfast_core::stash::stash_pop(&repo_path)
        .await
        .map_err(|e| e.to_string())
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
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
        .invoke_handler(tauri::generate_handler![
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
            get_diff_commit,
            get_diff_working_tree,
            get_diff_staged,
            pull,
            push,
            fetch_remote,
            open_repository,
            stash_push,
            stash_pop,
            open_terminal_at,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
