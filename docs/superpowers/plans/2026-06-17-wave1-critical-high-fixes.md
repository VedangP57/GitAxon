# GitAxon Wave 1 — Critical + High Severity Fixes

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix all 11 Critical and 22 High severity bugs from the enterprise audit — covering data loss, security, broken features, and silent failures across the Rust backend and SvelteKit frontend.

**Architecture:** Priority order — security (C1), then data-loss Rust fixes (C2–C11), then missing IPC events (H21–H22), then backend features (H6–H20), then frontend state bugs (H1–H5, H17–H18). Each task commits independently so the branch stays shippable.

**Tech Stack:** Rust (git2, tokio, notify), Tauri 2 IPC, SvelteKit 5 (Svelte 5 runes), `src-tauri/src/lib.rs` as the Tauri command layer, `ui/src/lib/store.ts` as the frontend state layer.

---

## File Map

| File | Tasks that modify it |
|------|---------------------|
| `src-tauri/Cargo.toml` | Task 1 |
| `src-tauri/src/lib.rs` | Tasks 1, 9, 10, 13 |
| `src/branches/mod.rs` | Tasks 2, 8 |
| `src/staging/mod.rs` | Tasks 3, 4, 6 |
| `src/diff/mod.rs` | Task 4 |
| `src/conflicts/mod.rs` | Task 5 |
| `src/stash/mod.rs` | Task 7 |
| `src/remotes/mod.rs` | Task 11 |
| `src/rebase/mod.rs` | Task 12 |
| `src/watcher/mod.rs` | Task 13 |
| `src/graph/mod.rs` | Task 14 |
| `ui/src/lib/store.ts` | Tasks 15, 22 |
| `ui/src/lib/tauri.ts` | Task 22 |
| `ui/src/components/ConflictResolver.svelte` | Tasks 15, 16 |
| `ui/src/components/RightPanel.svelte` | Tasks 17, 18 |
| `ui/src/components/PrReviewPanel.svelte` | Task 19 |
| `ui/src/components/StashManager.svelte` | Task 20 |
| `ui/src/components/RebasePanel.svelte` | Task 21 |

---

## Task 1: C1 — Keychain Token Storage

**Bugs fixed:** C1 (tokens stored as plaintext JSON on disk)  
**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs:916-1001`

- [ ] **Step 1.1 — Add keyring dependency**

In `src-tauri/Cargo.toml`, add after the `tauri-plugin-store = "2"` line:

```toml
keyring = "2"
```

- [ ] **Step 1.2 — Replace `get_token` with keychain lookup**

In `src-tauri/src/lib.rs`, replace lines 916–923:

```rust
// BEFORE:
fn get_token(app: &AppHandle, platform: &str) -> String {
    let store = app
        .store("tokens.json")
        .ok();
    store
        .and_then(|s| s.get(platform).and_then(|v| v.as_str().map(String::from)))
        .unwrap_or_default()
}
```

with:

```rust
fn get_token(_app: &AppHandle, platform: &str) -> String {
    keyring::Entry::new("gitaxon", platform)
        .ok()
        .and_then(|e| e.get_password().ok())
        .unwrap_or_default()
}
```

- [ ] **Step 1.3 — Replace `set_api_token` with keychain write + migration**

Replace lines 988–996 (`set_api_token` command):

```rust
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
```

- [ ] **Step 1.4 — Replace `get_api_token` command**

Replace lines 998–1001 (`get_api_token` command):

```rust
#[tauri::command]
async fn get_api_token(platform: String, _app: AppHandle) -> Result<String, String> {
    Ok(keyring::Entry::new("gitaxon", &platform)
        .ok()
        .and_then(|e| e.get_password().ok())
        .unwrap_or_default())
}
```

- [ ] **Step 1.5 — Verify the build compiles**

```bash
cd /Users/sarvadhisolution/Documents/Personal/GitAxon
cargo build -p gitaxon-tauri 2>&1 | grep -E "error|warning: unused"
```

Expected: no `error` lines (warnings about unused imports are OK for now).

- [ ] **Step 1.6 — Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/lib.rs
git commit -m "fix(security): store API tokens in OS keychain instead of plaintext JSON

Replaces tauri-plugin-store JSON storage with keyring crate for
macOS Keychain / Windows Credential Manager / libsecret. Migrates
any existing plaintext tokens on first write.

Fixes: C1"
```

---

## Task 2: C2 + C3 + C4 — Branch Safety Guards

**Bugs fixed:** C2 (FF merge on symbolic HEAD), C3 (merge on detached HEAD), C4 (force-delete current branch)  
**Files:**
- Modify: `src/branches/mod.rs`

- [ ] **Step 2.1 — Fix C4: prevent deleting the checked-out branch**

In `src/branches/mod.rs`, the `delete_branch` function starts at line 110. Replace the entire function body (lines 113–136) with:

```rust
pub async fn delete_branch(repo_path: &str, name: &str, force: bool) -> GitfastResult<()> {
    let path = repo_path.to_string();
    let name = name.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;
        let refname = format!("refs/heads/{}", name);

        // Guard: never delete the currently checked-out branch
        let head_name = repo
            .head()
            .ok()
            .and_then(|h| h.name().map(String::from))
            .unwrap_or_default();
        if head_name == refname {
            return Err(GitfastError::GitOperationFailed(
                format!("Cannot delete '{}': it is the currently checked-out branch.", name),
            ));
        }

        if force {
            let mut reference = repo
                .find_reference(&refname)
                .map_err(|_| GitfastError::BranchNotFound(name.clone()))?;
            reference.delete().map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        } else {
            let mut branch = repo
                .find_branch(&name, BranchType::Local)
                .map_err(|_| GitfastError::BranchNotFound(name.clone()))?;
            branch.delete().map_err(|e| {
                GitfastError::GitOperationFailed(format!(
                    "cannot delete branch '{}': {}. Use force=true to delete unmerged branches",
                    name, e
                ))
            })?;
        }
        Ok(())
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}
```

- [ ] **Step 2.2 — Fix C2 + C3: add detached HEAD guard and fix FF merge**

In `src/branches/mod.rs`, the `merge_branch` function starts at line 224. Replace the full function body with:

```rust
pub async fn merge_branch(repo_path: &str, branch_name: &str) -> GitfastResult<()> {
    let path = repo_path.to_string();
    let branch_name = branch_name.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;

        // C3: Guard against detached HEAD — merge would create an orphaned commit
        let head = repo.head().map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        if !head.is_branch() {
            return Err(GitfastError::GitOperationFailed(
                "Cannot merge: HEAD is detached. Checkout a branch first.".to_string(),
            ));
        }

        let branch_oid = repo
            .refname_to_id(&format!("refs/heads/{}", branch_name))
            .map_err(|_| GitfastError::BranchNotFound(branch_name.clone()))?;
        let branch_commit = repo
            .find_commit(branch_oid)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let annotated = repo
            .find_annotated_commit(branch_oid)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        let (analysis, _) = repo
            .merge_analysis(&[&annotated])
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        if analysis.is_fast_forward() {
            // C2: resolve symbolic HEAD → concrete branch ref before calling set_target
            let mut branch_ref = head
                .resolve()
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            branch_ref
                .set_target(branch_oid, "merge: Fast-forward")
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            repo.checkout_head(Some(&mut CheckoutBuilder::new()))
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            return Ok(());
        }

        let mut merge_opts = MergeOptions::new();
        let mut checkout = CheckoutBuilder::new();
        repo.merge(&[&annotated], Some(&mut merge_opts), Some(&mut checkout))
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        let mut index = repo.index().map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        if index.has_conflicts() {
            return Err(GitfastError::GitOperationFailed(
                "merge resulted in conflicts".to_string(),
            ));
        }

        let head_commit = repo
            .head()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
            .peel_to_commit()
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let sig = repo.signature().map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let tree_oid = index.write_tree().map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let tree = repo.find_tree(tree_oid).map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        let message = format!("Merge branch '{}'", branch_name);
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &message,
            &tree,
            &[&head_commit, &branch_commit],
        )
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        Ok(())
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}
```

- [ ] **Step 2.3 — Verify compile**

```bash
cargo build -p gitaxon 2>&1 | grep "^error"
```

Expected: no output (no errors).

- [ ] **Step 2.4 — Commit**

```bash
git add src/branches/mod.rs
git commit -m "fix(branches): guard against deleting current branch, detached HEAD merge, symbolic HEAD FF

- delete_branch: reject force-delete when branch is checked out (C4)
- merge_branch: return error when HEAD is detached (C3)
- merge_branch: resolve symbolic HEAD ref before set_target for FF merge (C2)

Fixes: C2, C3, C4"
```

---

## Task 3: C5 — Amend Preserves Original Author Date

**Bug fixed:** C5 (amend overwrites original author timestamp, invalidates GPG signatures)  
**Files:**
- Modify: `src/staging/mod.rs:329-351`

- [ ] **Step 3.1 — Fix `amend_commit` to preserve original author signature**

In `src/staging/mod.rs`, replace lines 329–351 (the entire `amend_commit` function):

```rust
/// Amends the last commit with current index + new message.
/// Preserves the original author name, email, and timestamp.
/// Only the committer timestamp is updated to now.
pub fn amend_commit(
    repo_path: &str,
    message: &str,
    author_name: &str,
    author_email: &str,
) -> Result<String, String> {
    let repo = open_repo(repo_path).map_err(|e| e.to_string())?;
    let mut index = repo.index().map_err(|e| e.to_string())?;
    let tree_id = index.write_tree().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;

    let head = repo.head().map_err(|e| e.to_string())?;
    let parent = head.peel_to_commit().map_err(|e| e.to_string())?;

    // Preserve the original author signature (name, email, AND timestamp).
    // Only committer gets the current time.
    let original_author = parent.author();
    let committer_now = git2::Signature::now(author_name, author_email)
        .map_err(|e| e.to_string())?;

    let oid = parent
        .amend(
            Some("HEAD"),
            Some(&original_author), // keep original author + date unchanged
            Some(&committer_now),   // committer updated to now
            None,                   // keep original encoding
            Some(message),
            Some(&tree),
        )
        .map_err(|e| e.to_string())?;

    Ok(oid.to_string())
}
```

- [ ] **Step 3.2 — Verify compile**

```bash
cargo build -p gitaxon 2>&1 | grep "^error"
```

Expected: no output.

- [ ] **Step 3.3 — Commit**

```bash
git add src/staging/mod.rs
git commit -m "fix(staging): amend_commit preserves original author date

Previously amend_commit created a new Signature::now() for both author
and committer, overwriting the original commit's author timestamp.
Now the original author signature is passed through unchanged; only
the committer timestamp is updated.

Fixes: C5"
```

---

## Task 4: C6 + C7 — Initial Commit Edge Cases

**Bugs fixed:** C6 (`diff_staged` panics on new repo with no commits), C7 (`unstage_file` panics on new repo)  
**Files:**
- Modify: `src/diff/mod.rs:250-298`
- Modify: `src/staging/mod.rs:222-240`

- [ ] **Step 4.1 — Fix `diff_staged` to handle unborn branch**

In `src/diff/mod.rs`, replace lines 250–268 (the `diff_staged` function):

```rust
/// Returns diff of index against HEAD (staged changes).
/// On a new repo with no commits (unborn HEAD), diffs against the empty tree.
pub async fn diff_staged(repo_path: &str) -> GitfastResult<Vec<DiffFile>> {
    let path = repo_path.to_string();
    tokio::task::spawn_blocking(move || {
        run_diff(&path, |repo| {
            let head_tree = match repo.head() {
                Ok(head) => Some(
                    head.peel_to_tree()
                        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?,
                ),
                Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
                Err(e) => return Err(GitfastError::GitOperationFailed(e.to_string())),
            };
            let mut diff = repo
                .diff_tree_to_index(head_tree.as_ref(), None, None)
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            process_diff(&mut diff)
        })
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}
```

- [ ] **Step 4.2 — Fix `get_staged_files_from_repo` for unborn branch**

In `src/diff/mod.rs`, replace lines 288–299 (`get_staged_files_from_repo`):

```rust
/// Returns staged files for a repository (sync, for use by other modules).
/// Returns empty vec on a new repo with no commits (unborn HEAD).
pub fn get_staged_files_from_repo(repo: &Repository) -> GitfastResult<Vec<DiffFile>> {
    let head_tree = match repo.head() {
        Ok(head) => Some(
            head.peel_to_tree()
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?,
        ),
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
        Err(e) => return Err(GitfastError::GitOperationFailed(e.to_string())),
    };
    let mut diff = repo
        .diff_tree_to_index(head_tree.as_ref(), None, None)
        .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
    process_diff(&mut diff)
}
```

- [ ] **Step 4.3 — Fix `unstage_file` to handle unborn branch**

In `src/staging/mod.rs`, replace lines 222–240 (the `unstage_file` function):

```rust
/// Unstages a file (resets in index to HEAD).
/// On a new repo with no commits (unborn HEAD), removes the entry from the index directly.
pub async fn unstage_file(repo_path: &str, file_path: &str) -> GitfastResult<()> {
    let path = repo_path.to_string();
    let file = file_path.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;
        match repo.head() {
            Ok(head) => {
                let obj = head
                    .peel(git2::ObjectType::Commit)
                    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
                repo.reset_default(Some(&obj), [Path::new(&file)])
                    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            }
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
                // No commits yet — remove the entry directly from the index
                let mut index = repo
                    .index()
                    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
                index
                    .remove_path(Path::new(&file))
                    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
                index
                    .write()
                    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            }
            Err(e) => return Err(GitfastError::GitOperationFailed(e.to_string())),
        }
        Ok(())
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}
```

- [ ] **Step 4.4 — Verify compile**

```bash
cargo build -p gitaxon 2>&1 | grep "^error"
```

Expected: no output.

- [ ] **Step 4.5 — Commit**

```bash
git add src/diff/mod.rs src/staging/mod.rs
git commit -m "fix(staging,diff): handle unborn branch for initial commit workflow

diff_staged and get_staged_files_from_repo now diff against the empty
tree when HEAD doesn't exist yet (new repo). unstage_file removes the
index entry directly instead of calling reset_default.

Fixes: C6, C7"
```

---

## Task 5: C9 + C10 — `continue_operation` Fixes

**Bugs fixed:** C9 (rebase continue hangs on GIT_EDITOR), C10 (merge continue without verifying conflicts resolved)  
**Files:**
- Modify: `src/conflicts/mod.rs:115-143`

- [ ] **Step 5.1 — Fix `continue_operation`**

In `src/conflicts/mod.rs`, replace lines 116–143 (the entire `continue_operation` function):

```rust
/// Continue the current operation (merge/rebase/cherry-pick).
pub fn continue_operation(repo_path: &str) -> Result<String, String> {
    let state = detect_operation_state(repo_path)?;

    if state.in_merge {
        // C10: verify all conflicts are resolved before committing
        let repo = crate::repo_pool::open_repo(repo_path).map_err(|e| e.to_string())?;
        let mut index = repo.index().map_err(|e| e.to_string())?;
        index.read(true).map_err(|e| e.to_string())?;
        if index.has_conflicts() {
            return Err(
                "There are still unresolved conflicts. Resolve all conflicts before continuing."
                    .to_string(),
            );
        }
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(["commit", "--no-edit"])
            .output()
            .map_err(|e| e.to_string())?;
        return if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        };
    }

    let (args, needs_editor_skip) = if state.in_rebase {
        // C9: set GIT_EDITOR=true to prevent the process from hanging when git
        // opens an editor (e.g., during non-reword steps after conflict resolution)
        (vec!["rebase", "--continue"], true)
    } else if state.in_cherry_pick {
        (vec!["cherry-pick", "--continue"], true)
    } else if state.in_revert {
        (vec!["revert", "--continue"], true)
    } else {
        return Err("No operation in progress".to_string());
    };

    let mut cmd = Command::new("git");
    cmd.current_dir(repo_path).args(&args);
    if needs_editor_skip {
        cmd.env("GIT_EDITOR", "true")
           .env("GIT_SEQUENCE_EDITOR", "true");
    }

    let output = cmd.output().map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}
```

- [ ] **Step 5.2 — Verify compile**

```bash
cargo build -p gitaxon 2>&1 | grep "^error"
```

Expected: no output.

- [ ] **Step 5.3 — Commit**

```bash
git add src/conflicts/mod.rs
git commit -m "fix(conflicts): continue_operation — prevent hang + verify conflicts resolved

- Rebase/cherry-pick/revert continue: set GIT_EDITOR=true so the process
  does not hang waiting for interactive editor input (C9)
- Merge continue: verify index has no conflicts before git commit --no-edit,
  returning a clear error if conflicts remain (C10)

Fixes: C9, C10"
```

---

## Task 6: C11 — `discard_all` Removes Untracked Files

**Bug fixed:** C11 (`discard_all` leaves untracked files behind)  
**Files:**
- Modify: `src/staging/mod.rs:370-377`

- [ ] **Step 6.1 — Fix `discard_all` to also remove untracked files**

In `src/staging/mod.rs`, replace lines 370–377 (the `discard_all` function):

```rust
/// Discard ALL unstaged changes: restore tracked files from HEAD and delete untracked files.
pub fn discard_all(repo: &Repository) -> Result<()> {
    // Restore all tracked files to HEAD state
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.force();
    repo.checkout_head(Some(&mut checkout))
        .map_err(|e| anyhow::anyhow!(e))?;

    // Also remove untracked files (new files not in HEAD)
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true).include_ignored(false);
    let statuses = repo
        .statuses(Some(&mut opts))
        .map_err(|e| anyhow::anyhow!(e))?;

    let workdir = repo
        .workdir()
        .ok_or_else(|| anyhow::anyhow!("No working directory"))?;

    for entry in statuses.iter() {
        if entry.status().is_wt_new() {
            if let Some(path_str) = entry.path() {
                let full_path = workdir.join(path_str);
                if full_path.is_dir() {
                    let _ = std::fs::remove_dir_all(&full_path);
                } else {
                    let _ = std::fs::remove_file(&full_path);
                }
            }
        }
    }

    Ok(())
}
```

- [ ] **Step 6.2 — Verify compile**

```bash
cargo build -p gitaxon 2>&1 | grep "^error"
```

Expected: no output.

- [ ] **Step 6.3 — Commit**

```bash
git add src/staging/mod.rs
git commit -m "fix(staging): discard_all now removes untracked files

Previously only restored tracked files via checkout_head(force).
Now also iterates status for wt_new entries and removes them.

Fixes: C11"
```

---

## Task 7: H20 — `stash_push` Includes Untracked Files

**Bug fixed:** H20 (stashing leaves new files behind)  
**Files:**
- Modify: `src/stash/mod.rs:57-77`

- [ ] **Step 7.1 — Add `--include-untracked` to stash_push**

In `src/stash/mod.rs`, replace lines 57–77 (the `stash_push` function):

```rust
pub fn stash_push(repo_path: &str, message: &str) -> Result<String, String> {
    let output = if message.trim().is_empty() {
        std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["stash", "push", "--include-untracked"])
            .output()
            .map_err(|e| e.to_string())?
    } else {
        std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["stash", "push", "--include-untracked", "-m", message])
            .output()
            .map_err(|e| e.to_string())?
    };

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}
```

- [ ] **Step 7.2 — Commit**

```bash
git add src/stash/mod.rs
git commit -m "fix(stash): stash_push includes untracked files

Without --include-untracked, new files were left behind after stashing.
Users who stashed and switched branches would find their new files still present.

Fixes: H20"
```

---

## Task 8: H9 — `checkout_branch` Stops Blocking on Unrelated Untracked Files

**Bug fixed:** H9 (checkout blocked on ANY untracked file even if unrelated to the branch)  
**Files:**
- Modify: `src/branches/mod.rs:158-221`

- [ ] **Step 8.1 — Remove the upfront status check from `checkout_branch`**

In `src/branches/mod.rs`, replace lines 158–221 (the entire `checkout_branch` function):

```rust
/// Checks out a branch. Lets git handle any real conflicts itself.
pub async fn checkout_branch(repo_path: &str, name: &str) -> GitfastResult<()> {
    let path = repo_path.to_string();
    let name = name.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = open_repo(&path)?;

        let (object, reference) = repo
            .revparse_ext(&name)
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;

        let target_ref = reference.as_ref().and_then(|r| r.name()).map(String::from);

        repo.checkout_tree(&object, Some(&mut CheckoutBuilder::new()))
            .map_err(|e| {
                // Provide a clear message for the most common real conflict:
                // an untracked file that would be overwritten
                let msg = e.to_string();
                if msg.contains("overwritten by checkout") || msg.contains("would be overwritten") {
                    GitfastError::GitOperationFailed(
                        format!("Cannot checkout: a local file would be overwritten. Commit or stash it first. ({})", msg)
                    )
                } else {
                    GitfastError::GitOperationFailed(msg)
                }
            })?;

        if let Some(r) = target_ref {
            if r.starts_with("refs/heads/") {
                repo.set_head(&r)
                    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            } else {
                repo.set_head_detached(object.id())
                    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
            }
        } else {
            repo.set_head_detached(object.id())
                .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?;
        }

        Ok(())
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}
```

Also remove the now-unused `StatusOptions` import if nothing else uses it. Check:

```bash
grep -n "StatusOptions" src/branches/mod.rs
```

If only one hit (in the old function), remove it from the `use` line at the top of the file. The `use git2::{build::CheckoutBuilder, BranchType, MergeOptions, Repository, StatusOptions}` becomes `use git2::{build::CheckoutBuilder, BranchType, MergeOptions, Repository}`.

- [ ] **Step 8.2 — Verify compile**

```bash
cargo build -p gitaxon 2>&1 | grep "^error"
```

Expected: no output.

- [ ] **Step 8.3 — Commit**

```bash
git add src/branches/mod.rs
git commit -m "fix(branches): checkout_branch stops blocking on unrelated untracked files

The previous upfront status scan blocked checkout when the user had ANY
new untracked file, even ones completely unrelated to the target branch.
Now git's own checkout_tree handles conflict detection, producing the
correct error only when a file would be overwritten.

Fixes: H9"
```

---

## Task 9: H7 + H8 — `get_repo_state` Limit Parameter + Partial Failure Resilience

**Bugs fixed:** H7 (hardcoded limit=500), H8 (full failure on any single sub-fetch error)  
**Files:**
- Modify: `src-tauri/src/lib.rs:1097-1157`

- [ ] **Step 9.1 — Add limit param and partial-failure resilience to `get_repo_state`**

In `src-tauri/src/lib.rs`, replace lines 1097–1138 (the `get_repo_state` command):

```rust
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
```

- [ ] **Step 9.2 — Add limit param to `get_graph_state`**

In `src-tauri/src/lib.rs`, replace lines 1143–1157 (the `get_graph_state` command):

```rust
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
```

- [ ] **Step 9.3 — Verify compile**

```bash
cargo build -p gitaxon-tauri 2>&1 | grep "^error"
```

Expected: no output.

- [ ] **Step 9.4 — Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "fix(ipc): get_repo_state/get_graph_state accept limit param, tolerate partial failures

- Both commands now accept an optional limit (default 500) so the frontend
  can request more commits as the user scrolls
- Sub-fetch failures return empty defaults instead of failing the whole
  response — a corrupted tag no longer prevents the repo from opening

Fixes: H7, H8"
```

---

## Task 10: H21 + H22 — Emit `git-state-changed` After Commit and Checkout

**Bugs fixed:** H21 (`create_commit` doesn't emit event), H22 (`checkout_branch` doesn't emit event)  
**Files:**
- Modify: `src-tauri/src/lib.rs:300-325, 678-687`

- [ ] **Step 10.1 — Add `app: AppHandle` param and emit to `create_commit`**

In `src-tauri/src/lib.rs`, replace lines 300–325 (the `create_commit` command):

```rust
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
```

- [ ] **Step 10.2 — Add `app: AppHandle` param and emit to `checkout_branch`**

In `src-tauri/src/lib.rs`, replace lines 678–687 (the `checkout_branch` command):

```rust
#[tauri::command]
async fn checkout_branch(repo_path: String, name: String, app: AppHandle) -> Result<(), String> {
    gitaxon::branches::checkout_branch(&repo_path, &name)
        .await
        .map_err(|e| e.to_string())?;

    invalidate_status_cache();
    let _ = app.emit("git-state-changed", repo_path.clone());

    Ok(())
}
```

- [ ] **Step 10.3 — Verify compile**

```bash
cargo build -p gitaxon-tauri 2>&1 | grep "^error"
```

Expected: no output.

- [ ] **Step 10.4 — Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "fix(ipc): emit git-state-changed after create_commit and checkout_branch

Previously the graph and branch list only refreshed incidentally when
the file watcher happened to fire. Now both commands emit git-state-changed
immediately on success, matching the pattern used by stage_file.

Fixes: H21, H22"
```

---

## Task 11: H10 + H11 — Fetch Reports Updated Refs + Push Sets Upstream

**Bugs fixed:** H10 (fetch always returns empty `updated_refs`), H11 (push doesn't set upstream tracking)  
**Files:**
- Modify: `src/remotes/mod.rs`

- [ ] **Step 11.1 — Fix `fetch` to report updated refs**

In `src/remotes/mod.rs`, replace the `fetch_remote_cmd` function (lines 122–135) and the `fetch` async function (lines 137–152) with:

```rust
fn fetch_remote_cmd(repo_path: &str, remote_name: &str) -> Result<Vec<String>, String> {
    // Run git fetch with --verbose to capture updated refs in stderr
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["fetch", "--verbose", remote_name])
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    // git fetch --verbose prints updated refs on stderr like:
    //   a1b2c3d..e4f5a6b  main -> origin/main
    let stderr = String::from_utf8_lossy(&output.stderr);
    let updated: Vec<String> = stderr
        .lines()
        .filter(|l| l.contains("->"))
        .map(|l| l.trim().to_string())
        .collect();

    Ok(updated)
}

/// Fetches from the given remote.
pub async fn fetch(repo_path: &str, remote_name: &str) -> GitfastResult<FetchResult> {
    let path = repo_path.to_string();
    let remote = remote_name.to_string();
    tokio::task::spawn_blocking(move || match fetch_remote_cmd(&path, &remote) {
        Ok(updated_refs) => Ok(FetchResult {
            remote,
            updated_refs,
            new_refs: Vec::new(),
        }),
        Err(err) => Err(GitfastError::GitOperationFailed(err)),
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}
```

- [ ] **Step 11.2 — Fix `push_branch` to set upstream tracking**

In `src/remotes/mod.rs`, replace lines 75–100 (`push_branch` function):

```rust
fn push_branch(
    repo_path: &str,
    remote_name: &str,
    branch_name: &str,
    force: bool,
) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.current_dir(repo_path)
        .arg("push")
        .arg("--set-upstream")  // configures tracking on first push
        .arg(remote_name)
        .arg(branch_name);

    if force {
        cmd.arg("--force");
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
```

- [ ] **Step 11.3 — Verify compile**

```bash
cargo build -p gitaxon 2>&1 | grep "^error"
```

Expected: no output.

- [ ] **Step 11.4 — Commit**

```bash
git add src/remotes/mod.rs
git commit -m "fix(remotes): fetch reports updated refs; push sets upstream tracking

- fetch: parse git fetch --verbose stderr to collect updated ref lines
- push_branch: add --set-upstream so tracking is configured on first push

Fixes: H10, H11"
```

---

## Task 12: H12 + H13 — Interactive Rebase: Full Hash + Reword Support

**Bugs fixed:** H12 (reword silently skipped), H13 (7-char short hash used in todo)  
**Files:**
- Modify: `src/rebase/mod.rs`

- [ ] **Step 12.1 — Fix H13: use full hash in todo + fix H12: reword message handling**

In `src/rebase/mod.rs`, replace lines 134–198 (both `start_interactive_rebase` and `continue_rebase`):

```rust
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

    // Write todo to a temp file; use GIT_SEQUENCE_EDITOR to copy it in place of git's generated todo
    let temp_dir = std::env::temp_dir();
    let todo_file = temp_dir.join("gitaxon_rebase_todo");
    fs::write(&todo_file, &todo_content).map_err(|e| e.to_string())?;

    let todo_path = todo_file.to_string_lossy().to_string();
    let editor_cmd = format!("cp {} ", todo_path);

    let output = Command::new("git")
        .current_dir(repo_path)
        .env("GIT_SEQUENCE_EDITOR", &editor_cmd)
        .args(["rebase", "-i", onto])
        .output()
        .map_err(|e| e.to_string())?;

    let _ = fs::remove_file(&todo_file);

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
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
```

- [ ] **Step 12.2 — Update the Tauri command signature for `continue_rebase`**

In `src-tauri/src/lib.rs`, find the `continue_rebase` Tauri command (search for `async fn continue_rebase`). Add the `new_message` parameter:

```rust
#[tauri::command]
async fn continue_rebase(repo_path: String, new_message: Option<String>) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        gitaxon::rebase::continue_rebase(&repo_path, new_message.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
}
```

- [ ] **Step 12.3 — Verify compile**

```bash
cargo build -p gitaxon-tauri 2>&1 | grep "^error"
```

Expected: no output.

- [ ] **Step 12.4 — Commit**

```bash
git add src/rebase/mod.rs src-tauri/src/lib.rs
git commit -m "fix(rebase): use full 40-char hash in todo; support reword message via continue

- start_interactive_rebase: use item.hash (40-char) instead of short_hash (H13)
- continue_rebase: accept optional new_message; when provided, feeds it to
  git via a temp editor script so reword steps are not silently skipped (H12)

Fixes: H12, H13"
```

---

## Task 13: H14 + H15 + H16 — Per-Repo Watcher + Sentinel Files

**Bugs fixed:** H14 (only one repo watched), H15 (`packed-refs` not watched), H16 (MERGE_HEAD/CHERRY_PICK_HEAD/REVERT_HEAD not watched)  
**Files:**
- Modify: `src/watcher/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 13.1 — Change WATCHER to per-repo HashMap in `watcher/mod.rs`**

In `src/watcher/mod.rs`, add `HashMap` to the existing imports if not present. Replace lines 36–37:

```rust
// BEFORE:
static WATCHER: Mutex<Option<RecommendedWatcher>> = Mutex::new(None);
static IS_RUNNING: AtomicBool = AtomicBool::new(false);
```

with:

```rust
use std::sync::Arc;
use std::collections::HashMap as StdHashMap;

struct WatcherHandle {
    _watcher: RecommendedWatcher,
    is_active: Arc<AtomicBool>,
}

static WATCHERS: Lazy<Mutex<StdHashMap<String, WatcherHandle>>> =
    Lazy::new(|| Mutex::new(StdHashMap::new()));
```

- [ ] **Step 13.2 — Rewrite `start_watching` signature + body**

Find `pub fn start_watching<F>(repo_path: &str, on_change: F)` (line 279) and replace the entire function with:

```rust
/// Start watching `repo_path`. When a worktree file changes, `on_worktree_change` fires.
/// When a .git state file changes (HEAD, index, refs, sentinels), `on_git_state_change` fires.
/// Multiple repos can be watched simultaneously.
pub fn start_watching<F, G>(
    repo_path: &str,
    on_worktree_change: F,
    on_git_state_change: G,
) -> Result<(), String>
where
    F: Fn() + Send + Sync + 'static,
    G: Fn() + Send + Sync + 'static,
{
    let is_active = Arc::new(AtomicBool::new(true));
    let is_active_clone = is_active.clone();

    let last_worktree = Arc::new(Mutex::new(Instant::now() - Duration::from_secs(10)));
    let last_gitstate = Arc::new(Mutex::new(Instant::now() - Duration::from_secs(10)));

    let last_worktree2 = last_worktree.clone();
    let last_gitstate2 = last_gitstate.clone();

    let real_repo_path = std::fs::canonicalize(repo_path)
        .unwrap_or_else(|_| Path::new(repo_path).to_path_buf());
    let repo_path_str = real_repo_path.to_string_lossy().to_string();

    let config = Config::default().with_poll_interval(Duration::from_millis(50));

    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            if !is_active_clone.load(Ordering::Relaxed) {
                return;
            }

            let Ok(event) = res else { return };

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

                let is_git_state = p.contains("/.git/HEAD")
                    || p.contains("/.git/index")
                    || p.contains("/.git/refs/heads/")
                    || p.contains("/.git/refs/remotes/")
                    || p.contains("/.git/packed-refs")
                    || p.contains("/.git/COMMIT_EDITMSG")
                    || p.contains("/.git/MERGE_HEAD")
                    || p.contains("/.git/CHERRY_PICK_HEAD")
                    || p.contains("/.git/REVERT_HEAD");

                let is_worktree = !p.contains("/.git/");

                if is_git_state {
                    let Ok(mut last) = last_gitstate2.lock() else { return };
                    if last.elapsed() > Duration::from_millis(50) {
                        *last = Instant::now();
                        drop(last);
                        on_git_state_change();
                    }
                    return;
                }

                if is_worktree {
                    let Ok(mut last) = last_worktree2.lock() else { return };
                    if last.elapsed() > Duration::from_millis(50) {
                        *last = Instant::now();
                        drop(last);
                        on_worktree_change();
                    }
                    return;
                }
            }
        },
        config,
    )
    .map_err(|e| e.to_string())?;

    // Watch .git state files
    let git_dir = real_repo_path.join(".git");
    if git_dir.exists() {
        for filename in &["HEAD", "index", "COMMIT_EDITMSG", "packed-refs",
                          "MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD"] {
            let p = git_dir.join(filename);
            // Watch even if file doesn't exist yet — notify will catch Create events
            let _ = watcher.watch(&p, RecursiveMode::NonRecursive);
        }
        let _ = watcher.watch(&git_dir.join("refs").join("heads"), RecursiveMode::Recursive);
        let _ = watcher.watch(&git_dir.join("refs").join("remotes"), RecursiveMode::Recursive);
    }

    let skip = [
        "node_modules", ".next", "dist", "build", "target",
        ".turbo", ".cache", "__pycache__", ".venv", "venv",
        "vendor", "Pods", "coverage", ".git", ".idea",
    ];
    watch_recursive_limited(&mut watcher, &real_repo_path, 0, 4, &skip);

    let mut handles = WATCHERS.lock().map_err(|_| "Failed to lock WATCHERS".to_string())?;
    handles.insert(repo_path_str.clone(), WatcherHandle { _watcher: watcher, is_active });
    log::info!("File watcher started for {}", repo_path_str);
    Ok(())
}
```

- [ ] **Step 13.3 — Rewrite `stop_watching` to take a repo path**

Find `pub fn stop_watching()` (line 438) and replace with:

```rust
/// Stop watching a specific repo. Other repos remain watched.
pub fn stop_watching(repo_path: &str) {
    let real = std::fs::canonicalize(repo_path)
        .unwrap_or_else(|_| Path::new(repo_path).to_path_buf());
    let key = real.to_string_lossy().to_string();

    if let Ok(mut handles) = WATCHERS.lock() {
        if let Some(handle) = handles.remove(&key) {
            handle.is_active.store(false, Ordering::Relaxed);
        }
    }
    log::info!("File watcher stopped for {}", key);
}
```

- [ ] **Step 13.4 — Update `start_file_watch` Tauri command to use two callbacks**

In `src-tauri/src/lib.rs`, replace lines 62–100 (the `start_file_watch` command):

```rust
#[tauri::command]
async fn start_file_watch(repo_path: String, app: AppHandle) -> Result<(), String> {
    let repo_path_str = repo_path.clone();

    // Channel for worktree file changes → emit worktree-changed
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
```

- [ ] **Step 13.5 — Verify compile**

```bash
cargo build -p gitaxon-tauri 2>&1 | grep "^error"
```

Expected: no output.

- [ ] **Step 13.6 — Commit**

```bash
git add src/watcher/mod.rs src-tauri/src/lib.rs
git commit -m "fix(watcher): per-repo watcher map; add sentinel file watches; emit git-state-changed

- WATCHER global replaced with WATCHERS HashMap keyed by repo path.
  Multiple repos can now be watched simultaneously without evicting each other. (H14)
- Added packed-refs, MERGE_HEAD, CHERRY_PICK_HEAD, REVERT_HEAD to watched paths.
  Sentinel file changes now trigger git-state-changed immediately. (H15, H16)
- start_watching now takes two callbacks: on_worktree_change and on_git_state_change.
  lib.rs routes them to worktree-changed and git-state-changed events respectively.

Fixes: H14, H15, H16"
```

---

## Task 14: H19 — Search Commits Include Parent Edges

**Bug fixed:** H19 (search result commits show as isolated dots with no connecting edges)  
**Files:**
- Modify: `src/graph/mod.rs`

- [ ] **Step 14.1 — Extend search results with immediate parents for edge generation**

In `src/graph/mod.rs`, find the `search_commits` function (line 274). After the loop that builds `commits` (after line 355, before `Ok(commits)`), add parent extension:

```rust
    // H19: extend results with immediate parents of matching commits so edges connect.
    // Parents outside the filtered set have no row in sha_to_idx, producing isolated dots.
    let matching_hashes: std::collections::HashSet<String> =
        commits.iter().map(|c| c.hash.clone()).collect();
    let mut extended = commits;

    // Collect parent hashes not already in results
    let missing_parents: Vec<String> = extended
        .iter()
        .flat_map(|c| c.parent_hashes.iter().cloned())
        .filter(|h| !matching_hashes.contains(h))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Fetch each missing parent via git log
    for parent_hash in missing_parents {
        let output = std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["log", "-1", "--format=%H|%h|%s|%an|%ae|%ct|%P", &parent_hash])
            .output();
        if let Ok(out) = output {
            if out.status.success() {
                let line = String::from_utf8_lossy(&out.stdout);
                let line = line.trim();
                if !line.is_empty() {
                    let parts: Vec<&str> = line.splitn(7, '|').collect();
                    if parts.len() >= 6 {
                        let parent_hashes: Vec<String> = if parts.len() >= 7 && !parts[6].is_empty() {
                            parts[6].split(' ').map(|s| s.to_string()).collect()
                        } else {
                            vec![]
                        };
                        extended.push(crate::cache::CommitNode {
                            hash: parts[0].to_string(),
                            short_hash: parts[1].to_string(),
                            message: parts[2].to_string(),
                            author_name: parts[3].to_string(),
                            author_email: parts[4].to_string(),
                            timestamp: parts[5].parse::<i64>().unwrap_or(0),
                            parent_hashes,
                        });
                    }
                }
            }
        }
    }

    Ok(extended)
```

The full end of `search_commits` should now read:

```rust
    // ... (existing commit parsing loop) ...

    // H19: extend with parents (see block above)
    let matching_hashes: std::collections::HashSet<String> =
        commits.iter().map(|c| c.hash.clone()).collect();
    let mut extended = commits;

    let missing_parents: Vec<String> = extended
        .iter()
        .flat_map(|c| c.parent_hashes.iter().cloned())
        .filter(|h| !matching_hashes.contains(h))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    for parent_hash in missing_parents {
        let output = std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["log", "-1", "--format=%H|%h|%s|%an|%ae|%ct|%P", &parent_hash])
            .output();
        if let Ok(out) = output {
            if out.status.success() {
                let line = String::from_utf8_lossy(&out.stdout);
                let line = line.trim();
                if !line.is_empty() {
                    let parts: Vec<&str> = line.splitn(7, '|').collect();
                    if parts.len() >= 6 {
                        let parent_hashes_vec: Vec<String> = if parts.len() >= 7 && !parts[6].is_empty() {
                            parts[6].split(' ').map(|s| s.to_string()).collect()
                        } else {
                            vec![]
                        };
                        extended.push(crate::cache::CommitNode {
                            hash: parts[0].to_string(),
                            short_hash: parts[1].to_string(),
                            message: parts[2].to_string(),
                            author_name: parts[3].to_string(),
                            author_email: parts[4].to_string(),
                            timestamp: parts[5].parse::<i64>().unwrap_or(0),
                            parent_hashes: parent_hashes_vec,
                        });
                    }
                }
            }
        }
    }

    Ok(extended)
}
```

- [ ] **Step 14.2 — Verify compile**

```bash
cargo build -p gitaxon 2>&1 | grep "^error"
```

Expected: no output.

- [ ] **Step 14.3 — Commit**

```bash
git add src/graph/mod.rs
git commit -m "fix(graph): search_commits includes immediate parents for edge generation

Search results were showing isolated dots because parent commits outside
the filter set had no sha_to_idx entry, so generate_edges produced no
connecting lines. Now each matching commit's parents are fetched and
included in the result so the lane algorithm can draw connecting edges.

Fixes: H19"
```

---

## Task 15: C8 — `openConflictResolver` Threads `filePath` to Component

**Bug fixed:** C8 (`openConflictResolver(filePath)` ignores its argument)  
**Files:**
- Modify: `ui/src/lib/store.ts:523-527`
- Modify: `ui/src/components/ConflictResolver.svelte`

- [ ] **Step 15.1 — Add `conflictFilePathStore` to store.ts**

In `ui/src/lib/store.ts`, find where other `writable` stores are declared (around line 79, alongside `centerViewStore`). Add a new store:

```ts
const conflictFilePathStore = writable<string | null>(null);
export const conflictFilePath = { subscribe: conflictFilePathStore.subscribe };
```

- [ ] **Step 15.2 — Update `openConflictResolver` to set the store**

In `ui/src/lib/store.ts`, replace lines 523–527:

```ts
// BEFORE:
export function openConflictResolver(filePath: string): void {
    centerViewStore.set('conflict');
    // The ConflictResolver component will load the file via its own method
}
```

with:

```ts
export function openConflictResolver(filePath: string): void {
    conflictFilePathStore.set(filePath);
    centerViewStore.set('conflict');
}
```

- [ ] **Step 15.3 — Update ConflictResolver.svelte to subscribe to the store**

In `ui/src/components/ConflictResolver.svelte`, update the `<script>` imports block to add `conflictFilePath`:

```svelte
<script lang="ts">
    import { get } from 'svelte/store';
    import { currentRepo, closeDiff, conflictFilePath } from '$lib/store';
    import { getConflictFile, resolveConflict, continueOperation, abortOperation } from '$lib/tauri';
    import { showToast } from '$lib/toast';
    import type { ConflictFile } from '$lib/types';

    let conflictFile = $state<ConflictFile | null>(null);
    let resolvedContent = $state('');
    let isLoading = $state(false);
    let filePath = $state('');

    // Auto-load when the store sets a new conflict file path
    $effect(() => {
        const path = $conflictFilePath;
        if (path && path !== filePath) {
            loadConflict(path);
        }
    });

    export async function loadConflict(path: string) {
        // ... existing implementation unchanged ...
    }
    // ... rest of existing script unchanged ...
</script>
```

The `$effect` block replaces the current dependency on `loadConflict` being called externally with an unknown caller.

- [ ] **Step 15.4 — Verify TypeScript**

```bash
cd /Users/sarvadhisolution/Documents/Personal/GitAxon/ui
npx tsc --noEmit 2>&1 | grep "error TS"
```

Expected: no `error TS` lines.

- [ ] **Step 15.5 — Commit**

```bash
git add ui/src/lib/store.ts ui/src/components/ConflictResolver.svelte
git commit -m "fix(conflict): openConflictResolver now threads filePath to ConflictResolver component

Previously openConflictResolver() ignored its filePath argument. Now it writes
to conflictFilePathStore, and ConflictResolver subscribes via \$effect to
auto-load the file whenever the store changes.

Fixes: C8"
```

---

## Task 16: H17 — ConflictResolver Back Button Confirmation

**Bug fixed:** H17 (back button discards edits without any warning)  
**Files:**
- Modify: `ui/src/components/ConflictResolver.svelte`

- [ ] **Step 16.1 — Add isDirty tracking and confirmation guard**

In `ui/src/components/ConflictResolver.svelte`, add `isDirty` after the existing state declarations (around line 11):

```ts
let isDirty = $derived(
    resolvedContent !== '' && resolvedContent !== (conflictFile?.merged ?? '')
);
```

Then find the back button at line 80:

```svelte
<button class="cr-back" onclick={() => closeDiff()}>←</button>
```

Replace with:

```svelte
<button class="cr-back" onclick={handleBack}>←</button>
```

Add the `handleBack` function in the `<script>` block (before `lineCount`):

```ts
function handleBack() {
    if (isDirty) {
        if (!confirm('Discard your changes to the conflict resolution? This cannot be undone.')) {
            return;
        }
    }
    closeDiff();
}
```

- [ ] **Step 16.2 — Verify TypeScript**

```bash
cd /Users/sarvadhisolution/Documents/Personal/GitAxon/ui
npx tsc --noEmit 2>&1 | grep "error TS"
```

Expected: no `error TS` lines.

- [ ] **Step 16.3 — Commit**

```bash
git add ui/src/components/ConflictResolver.svelte
git commit -m "fix(conflict): back button confirms before discarding unsaved resolution edits

Previously clicking ← during active conflict editing silently discarded
all changes. Now a confirmation dialog appears when resolvedContent
differs from the original merged content.

Fixes: H17"
```

---

## Task 17: H1 — Amend Toast Fix

**Bug fixed:** H1 (amend toast always says "Commit created" because `amend` is reset before the toast reads it)  
**Files:**
- Modify: `ui/src/components/RightPanel.svelte:283-308`

- [ ] **Step 17.1 — Capture amend state before resetting**

In `ui/src/components/RightPanel.svelte`, find lines 299–302 inside `handleCommit`:

```ts
// BEFORE:
            amend = false;
            showToast(amend ? "Commit amended" : "Commit created successfully", "success");
```

Replace with:

```ts
            const wasAmend = amend;
            amend = false;
            showToast(wasAmend ? "Commit amended successfully" : "Commit created successfully", "success");
```

- [ ] **Step 17.2 — Commit**

```bash
git add ui/src/components/RightPanel.svelte
git commit -m "fix(ui): amend toast now correctly reads amend state before resetting it

amend was set to false before the toast message read it, so the message
always showed 'Commit created'. Now captured as wasAmend first.

Fixes: H1"
```

---

## Task 18: H2 — `handleMenuStage` Always Calls `stageFile`

**Bug fixed:** H2 (right-click "Unstage" calls `stageFile` instead of `unstageFile`)  
**Files:**
- Modify: `ui/src/components/RightPanel.svelte:262-268`

- [ ] **Step 18.1 — Fix `handleMenuStage` to check `entry.staged`**

In `ui/src/components/RightPanel.svelte`, replace lines 262–268 (the `handleMenuStage` function):

```ts
// BEFORE:
    function handleMenuStage(entry: (StatusEntry | IndexEntry) | null) {
        closeFileMenu();
        if (!entry) return;
        const repo = $currentRepo;
        if (!repo) return;
        void withStagingUi(entry.path, () => stageFile(repo, entry.path));
    }
```

with:

```ts
    function handleMenuStage(entry: (StatusEntry | IndexEntry) | null) {
        closeFileMenu();
        if (!entry) return;
        const repo = $currentRepo;
        if (!repo) return;
        if ('staged' in entry && entry.staged) {
            void withStagingUi(entry.path, () => unstageFile(repo, entry.path));
        } else {
            void withStagingUi(entry.path, () => stageFile(repo, entry.path));
        }
    }
```

- [ ] **Step 18.2 — Verify the menu label matches the action**

Find the menu item that calls `handleMenuStage` (around line 669). The label should read dynamically. Find:

```svelte
<button class="menu-item" onclick={() => { handleMenuStage(fileMenuEntry); closeFileMenu(); }}>
```

Update to show the correct label:

```svelte
<button class="menu-item" onclick={() => { handleMenuStage(fileMenuEntry); closeFileMenu(); }}>
    {fileMenuEntry && 'staged' in fileMenuEntry && fileMenuEntry.staged ? 'Unstage file' : 'Stage file'}
</button>
```

- [ ] **Step 18.3 — Verify TypeScript**

```bash
cd /Users/sarvadhisolution/Documents/Personal/GitAxon/ui
npx tsc --noEmit 2>&1 | grep "error TS"
```

Expected: no `error TS` lines.

- [ ] **Step 18.4 — Commit**

```bash
git add ui/src/components/RightPanel.svelte
git commit -m "fix(ui): handleMenuStage calls unstageFile when entry is already staged

The context menu 'Unstage file' option was always calling stageFile.
Now checks entry.staged to call the correct function. Menu label also
updates dynamically to show Stage vs Unstage.

Fixes: H2"
```

---

## Task 19: H3 + H4 — PR Panel: Stale `selectedFile` + Race Condition

**Bugs fixed:** H3 (`selectedFile` not cleared when switching PRs), H4 (race condition on rapid PR switching)  
**Files:**
- Modify: `ui/src/components/PrReviewPanel.svelte:24-45`

- [ ] **Step 19.1 — Fix `loadPrData` to reset selectedFile and guard against races**

In `ui/src/components/PrReviewPanel.svelte`, add a sequence counter after the `let selectedFile` declaration (around line 14):

```ts
let selectedFile = $state<PrFile | null>(null);
let loadSeq = 0;  // ← add this line
```

Then replace lines 24–45 (the `loadPrData` function):

```ts
async function loadPrData(num: number) {
    const seq = ++loadSeq;  // snapshot current sequence number

    // H3: reset stale file immediately before any async work
    selectedFile = null;
    files = [];
    comments = [];

    const repo = get(currentRepo);
    if (!repo) return;

    const prList = get(prs);
    pr = prList.find(p => p.number === num) ?? null;

    isLoading = true;
    try {
        const [f, c] = await Promise.all([
            getPrFiles(repo, num),
            getPrComments(repo, num),
        ]);
        // H4: discard result if a newer call has already started
        if (seq !== loadSeq) return;
        files = f;
        comments = c;
    } catch (e) {
        if (seq !== loadSeq) return;
        showToast(String(e), 'error');
    } finally {
        if (seq === loadSeq) isLoading = false;
    }
}
```

- [ ] **Step 19.2 — Verify TypeScript**

```bash
cd /Users/sarvadhisolution/Documents/Personal/GitAxon/ui
npx tsc --noEmit 2>&1 | grep "error TS"
```

Expected: no `error TS` lines.

- [ ] **Step 19.3 — Commit**

```bash
git add ui/src/components/PrReviewPanel.svelte
git commit -m "fix(pr): clear selectedFile on PR switch; guard against rapid-switch race condition

- Reset selectedFile, files, comments before any async work so the old PR's
  data never shows under the new PR's context (H3)
- Added loadSeq counter: async results from a superseded call are discarded,
  preventing slower responses from winning over faster ones (H4)

Fixes: H3, H4"
```

---

## Task 20: H5 — StashManager `get()` Not Reactive in Svelte 5

**Bug fixed:** H5 (stash list never refreshes when switching repos)  
**Files:**
- Modify: `ui/src/components/StashManager.svelte`

- [ ] **Step 20.1 — Replace `get(currentRepo)` with reactive `$currentRepo` in the $effect**

In `ui/src/components/StashManager.svelte`, find the initial load block (around lines 107–109):

```ts
// BEFORE:
    if (get(currentRepo)) {
        loadStashes();
```

Replace with:

```ts
    $effect(() => {
        if ($currentRepo) {
            loadStashes();
        }
    });
```

Remove the old non-reactive initialization block that uses `get(currentRepo)` (lines 107–109) if it exists as a standalone call. The `$effect` above replaces it and will re-run automatically when `currentRepo` changes.

Also update `loadStashes` to use the reactive store directly instead of `get()`. At the top of `loadStashes` (line 25), replace:

```ts
// BEFORE:
    async function loadStashes() {
        const repo = get(currentRepo);
```

with:

```ts
    async function loadStashes() {
        const repo = $currentRepo;
```

Do the same for all other functions in StashManager.svelte that call `get(currentRepo)` inside the same component — replace each `get(currentRepo)` with `$currentRepo` for consistency.

- [ ] **Step 20.2 — Verify TypeScript**

```bash
cd /Users/sarvadhisolution/Documents/Personal/GitAxon/ui
npx tsc --noEmit 2>&1 | grep "error TS"
```

Expected: no `error TS` lines.

- [ ] **Step 20.3 — Commit**

```bash
git add ui/src/components/StashManager.svelte
git commit -m "fix(stash): StashManager reactively reloads on repo switch using Svelte 5 runes

In Svelte 5 runes mode, get() inside \$effect takes a snapshot and is not
tracked. Replaced all get(currentRepo) with the reactive \$currentRepo sigil
so the \$effect re-runs when the active repo changes.

Fixes: H5"
```

---

## Task 21: H18 — RebasePanel: Clear `todoItems` Before New Rebase Load

**Bug fixed:** H18 (previous rebase list shows while a new one is loading)  
**Files:**
- Modify: `ui/src/components/RebasePanel.svelte`

- [ ] **Step 21.1 — Clear `todoItems` at the start of `setupRebase`**

In `ui/src/components/RebasePanel.svelte`, find `setupRebase` (line 26). The function starts:

```ts
    export async function setupRebase(targetHash: string) {
        const repo = $currentRepo;
        if (!repo) return;
        mode = 'setup';
        isWorking = true;
```

Add `todoItems = [];` immediately after `isWorking = true;`:

```ts
    export async function setupRebase(targetHash: string) {
        const repo = $currentRepo;
        if (!repo) return;
        mode = 'setup';
        isWorking = true;
        todoItems = [];  // ← add this line — clear stale list before async fetch
        try {
```

- [ ] **Step 21.2 — Commit**

```bash
git add ui/src/components/RebasePanel.svelte
git commit -m "fix(rebase): clear todoItems immediately when setupRebase is called

The previous rebase list was visible during the async fetch of the new one.
Adding todoItems = [] before the try block clears the UI instantly.

Fixes: H18"
```

---

## Task 22: H6 — Pass Growing Limit from Frontend to `get_graph_state`

**Bug fixed:** H6 (paginated graph loads sort and assign lanes per-page independently, breaking continuity)  
**Files:**
- Modify: `ui/src/lib/tauri.ts`
- Modify: `ui/src/lib/store.ts`

- [ ] **Step 22.1 — Update `getGraphState` IPC binding to accept limit**

In `ui/src/lib/tauri.ts`, find the `getGraphState` function. It currently calls:

```ts
invoke('get_graph_state', { repoPath })
```

Update it to accept and pass a `limit` parameter:

```ts
export async function getGraphState(repoPath: string, limit?: number): Promise<GraphStateResponse> {
    const raw = await invoke<{ commits: string; branches: string; tags: string }>(
        'get_graph_state',
        { repoPath, limit }
    );
    return {
        commits: JSON.parse(raw.commits),
        branches: JSON.parse(raw.branches),
        tags: JSON.parse(raw.tags),
    };
}
```

Also update `getRepoState` if it has the same pattern:

```ts
export async function getRepoState(repoPath: string, limit?: number): Promise<RepoStateResponse> {
    const raw = await invoke<{ commits: string; branches: string; tags: string; status: string }>(
        'get_repo_state',
        { repoPath, limit }
    );
    return {
        commits: JSON.parse(raw.commits),
        branches: JSON.parse(raw.branches),
        tags: JSON.parse(raw.tags),
        status: JSON.parse(raw.status),
    };
}
```

- [ ] **Step 22.2 — Pass growing limit in `scheduleGraphReload` and `loadMoreCommits`**

In `ui/src/lib/store.ts`, find `scheduleGraphReload` (search for it by name). The function calls `getGraphState(repo)`. Update it to pass the current commit count as the limit:

```ts
async function doGraphReload(repo: string) {
    const currentLen = get(commitsStore).length;
    const limit = Math.max(500, currentLen + 100); // always load at least 500; keep all currently loaded + 100 more
    const { commits, branches, tags } = await getGraphState(repo, limit);
    commitsStore.set(commits);
    branchesStore.set(branches);
    tagsStore.set(tags);
}
```

Find `loadMoreCommits` in `store.ts`. It currently calls `getCommits(repo, currentLength + 500, 0)`. Update to also pass through `getGraphState` with the growing limit so lanes are computed on the full set:

```ts
export async function loadMoreCommits(): Promise<void> {
    const repo = get(currentRepoStore);
    if (!repo || !get(hasMoreStore)) return;
    const currentLen = get(commitsStore).length;
    const newLimit = currentLen + 500;
    const { commits, branches, tags } = await getGraphState(repo, newLimit);
    commitsStore.set(commits);
    branchesStore.set(branches);
    tagsStore.set(tags);
    hasMoreStore.set(commits.length >= newLimit);
}
```

- [ ] **Step 22.3 — Verify TypeScript**

```bash
cd /Users/sarvadhisolution/Documents/Personal/GitAxon/ui
npx tsc --noEmit 2>&1 | grep "error TS"
```

Expected: no `error TS` lines.

- [ ] **Step 22.4 — Commit**

```bash
git add ui/src/lib/tauri.ts ui/src/lib/store.ts
git commit -m "fix(graph): pass growing limit to get_graph_state to fix per-page lane assignment

Previously loadMoreCommits and scheduleGraphReload always fetched with limit=500,
causing each page to sort and assign lanes independently. Now the full
growing window (currentLen + 500) is passed so lane assignment covers all
commits loaded so far in one continuous pass.

Fixes: H6"
```

---

## Self-Review Checklist

**Spec coverage:**
- C1 ✓ Task 1, C2 ✓ Task 2, C3 ✓ Task 2, C4 ✓ Task 2, C5 ✓ Task 3
- C6 ✓ Task 4, C7 ✓ Task 4, C8 ✓ Task 15, C9 ✓ Task 5, C10 ✓ Task 5, C11 ✓ Task 6
- H1 ✓ Task 17, H2 ✓ Task 18, H3 ✓ Task 19, H4 ✓ Task 19, H5 ✓ Task 20
- H6 ✓ Task 22, H7 ✓ Task 9, H8 ✓ Task 9, H9 ✓ Task 8, H10 ✓ Task 11
- H11 ✓ Task 11, H12 ✓ Task 12, H13 ✓ Task 12, H14 ✓ Task 13, H15 ✓ Task 13
- H16 ✓ Task 13, H17 ✓ Task 16, H18 ✓ Task 21, H19 ✓ Task 14
- H20 ✓ Task 7, H21 ✓ Task 10, H22 ✓ Task 10

**Execution order note:** Tasks 1–14 are Rust backend tasks and can be executed in any order. Tasks 15–22 are frontend tasks. Task 13 (watcher) must come before testing H15/H16. Task 15 (C8 store + ConflictResolver) must come before Task 16 (H17) since both touch ConflictResolver.svelte.
