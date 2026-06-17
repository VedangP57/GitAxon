# GitAxon Enterprise Bug Fixes — Design Spec

**Date:** 2026-06-17  
**Branch:** features/changes-in-toster-ui  
**Scope:** 83 confirmed bugs + 26 enterprise gaps = 109 total fixes  
**Strategy:** Option A — Priority-first (Critical → High → Medium → Low → Enterprise)

---

## Overview

Four implementation waves, each independently shippable. Every wave runs concurrent Rust and Svelte tracks to maximize parallel delivery.

| Wave | Focus | Items | Target files |
|------|-------|-------|--------------|
| 1 | Critical — data loss, security, crashes | C1–C11, H1–H22 | ~20 |
| 2 | UX & state polish | M1–M34, L1–L17 | ~12 |
| 3 | Performance & architecture | P1–P6 | ~6 |
| 4 | Enterprise features | E1–E26 | ~25 new/changed |

---

## Wave 1 — Critical + High Severity

### C1 — API tokens stored as plaintext JSON

**File:** `src-tauri/src/lib.rs:989`  
**Fix:** Replace `tauri-plugin-store` token storage with OS Keychain via `keyring` crate.  
Add to `Cargo.toml`: `keyring = "2"`. In `lib.rs`, replace `get_token`/`store_token` JSON reads with `keyring::Entry::new("gitaxon", platform)?.get_password()` / `.set_password(token)`. Remove `tokens.json` file creation. Existing plaintext tokens should be migrated: on first read, if keychain is empty and JSON file exists, import and delete it.

### C2 — Fast-forward merge calls `set_target()` on symbolic HEAD

**File:** `src/branches/mod.rs:243`  
**Fix:** Resolve HEAD to the concrete branch ref before updating:
```rust
let head = repo.head()?;
let mut branch_ref = head.resolve()?;  // resolves HEAD → refs/heads/main
branch_ref.set_target(branch_oid, "merge: Fast-forward")?;
```
`resolve()` returns the underlying direct reference; `set_target` is safe on it.

### C3 — Merge on detached HEAD silently orphans commit

**File:** `src/branches/mod.rs:265`  
**Fix:** Before starting the merge, check `repo.head()?.is_branch()`. If false, return `Err("Cannot merge: HEAD is detached. Checkout a branch first.")`.

### C4 — Force-delete allows deleting the checked-out branch

**File:** `src/branches/mod.rs:110`  
**Fix:** Before deletion, compare the branch refname against HEAD:
```rust
let head_name = repo.head()?.name().unwrap_or("").to_string();
let branch_refname = format!("refs/heads/{}", name);
if head_name == branch_refname {
    return Err("Cannot delete the currently checked-out branch".into());
}
```
This applies to both `force=true` and `force=false` paths.

### C5 — Amend overwrites original author timestamp

**File:** `src/staging/mod.rs:344`  
**Fix:** Use `commit.amend()` (not `repo.commit()`) so git2 preserves the author field from the original. Pass `Some(&original_author)` to keep the author signature unchanged; only the committer gets `now()`:
```rust
let head_commit = repo.head()?.peel_to_commit()?;
let original_author = head_commit.author();
let committer_now = git2::Signature::now(
    original_author.name().unwrap_or(""),
    original_author.email().unwrap_or(""),
)?;
head_commit.amend(
    Some("HEAD"),
    Some(&original_author),  // preserve original author + date
    Some(&committer_now),    // committer gets current time
    None,                    // keep encoding
    Some(&new_message),
    Some(&new_tree),
)?;
```

### C6 — `diff_staged` panics on first commit (no HEAD)

**File:** `src/diff/mod.rs:250`  
**Fix:** Catch `ErrorCode::UnbornBranch`:
```rust
let tree = match repo.head() {
    Ok(head) => Some(head.peel_to_tree()?),
    Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
    Err(e) => return Err(e.into()),
};
let diff = repo.diff_tree_to_index(tree.as_ref(), None, None)?;
```
`diff_tree_to_index` with `None` as tree diffs against the empty tree, which is correct for initial commit staging.

### C7 — `unstage_file` panics on first commit (no HEAD)

**File:** `src/staging/mod.rs:222`  
**Fix:** Same pattern as C6. If `UnbornBranch`, fall back to `index.remove_path(Path::new(path))?` to remove the entry directly from the index without needing a HEAD tree.

### C8 — `openConflictResolver(filePath)` drops its argument

**File:** `ui/src/lib/store.ts:524`  
**Fix:** Add a dedicated store and thread the path through:
```ts
export const conflictFilePathStore = writable<string | null>(null);

export function openConflictResolver(filePath: string) {
    conflictFilePathStore.set(filePath);
    centerView.set('conflict');
}
```
In `ConflictResolver.svelte`, subscribe to `conflictFilePathStore` and auto-call `loadFile($conflictFilePathStore)` in a `$effect`.

### C9 — `continue_operation` for rebase hangs on GIT_EDITOR

**File:** `src/conflicts/mod.rs:119`  
**Fix:** Add `GIT_EDITOR=true` to the `git rebase --continue` Command env, identical to `rebase/mod.rs:185`:
```rust
Command::new("git")
    .args(["rebase", "--continue"])
    .env("GIT_EDITOR", "true")
    .env("GIT_SEQUENCE_EDITOR", "true")
    ...
```

### C10 — `continue_operation` for merge doesn't verify conflicts resolved

**File:** `src/conflicts/mod.rs:125`  
**Fix:** Before running `git commit --no-edit`, verify the index is clean:
```rust
let mut index = repo.index()?;
index.read(true)?;
if index.has_conflicts() {
    return Err("There are still unresolved conflicts. Resolve all conflicts before continuing.".into());
}
```

### C11 — `discard_all` doesn't remove untracked files

**File:** `src/staging/mod.rs:371`  
**Fix:** After `checkout_head(force)`, collect all untracked entries and remove them:
```rust
let statuses = repo.statuses(None)?;
for entry in statuses.iter() {
    if entry.status().is_wt_new() {
        let path = repo_path.join(entry.path().unwrap_or(""));
        if path.is_dir() {
            std::fs::remove_dir_all(&path).ok();
        } else {
            std::fs::remove_file(&path).ok();
        }
    }
}
```
The frontend must show a confirmation modal before calling this command (already does for tracked files; extend the same modal text to mention untracked files).

---

### H1 — Amend toast always says "Commit created"

**File:** `ui/src/components/RightPanel.svelte`  
**Fix:** Capture the amend state before resetting it:
```ts
const wasAmend = amend;
amend = false;
showToast(wasAmend ? "Commit amended successfully" : "Commit created successfully", "success");
```

### H2 — Context menu "Stage" always calls `stageFile`

**File:** `ui/src/components/RightPanel.svelte`  
**Fix:** In `handleMenuStage`, check `entry.staged`:
```ts
async function handleMenuStage(entry: StatusEntry) {
    if (entry.staged) {
        await unstageFile(entry.path);
    } else {
        await stageFile(entry.path);
    }
}
```
Also update the menu label to show "Unstage" when `entry.staged` is true.

### H3 — `selectedFile` stale when switching PRs

**File:** `ui/src/components/PrReviewPanel.svelte`  
**Fix:** Reset at the top of `loadPrData`:
```ts
async function loadPrData(num: number) {
    selectedFile = null;   // ← add this
    files = [];
    comments = [];
    ...
}
```

### H4 — Race condition on rapid PR switching

**File:** `ui/src/components/PrReviewPanel.svelte`  
**Fix:** Add a sequence counter to detect stale responses:
```ts
let loadSeq = 0;
async function loadPrData(num: number) {
    const seq = ++loadSeq;
    selectedFile = null;
    ...
    const [f, c] = await Promise.all([getPrFiles(...), getPrComments(...)]);
    if (seq !== loadSeq) return;  // stale — a newer call is in progress
    files = f;
    comments = c;
}
```

### H5 — StashManager `get()` not reactive in Svelte 5

**File:** `ui/src/components/StashManager.svelte`  
**Fix:** Replace the non-reactive `get()` call with the runes sigil:
```ts
// BEFORE:
$effect(() => { if (get(currentRepo)) loadStashes(); });

// AFTER:
$effect(() => { if ($currentRepo) loadStashes(); });
```
`$currentRepo` is read through the runes reactive tracker; the effect re-runs when the store changes.

### H6 — Per-page independent lane assignment breaks graph

**File:** `src/graph/mod.rs:195`  
**Fix:** For the growing-window approach used by the frontend (`loadMoreCommits` fetches 0..currentLength+500), ensure `get_laned_commits_json` receives `offset=0` always and `limit=requested_total`. Pagination is then pure slicing on the already-sorted result. The Rust function already sorts and assigns lanes on the full slice it receives; the bug is that the frontend calls `get_commits(repo, len+500, 0)` but `get_repo_state` / `get_graph_state` hardcode their own limits. See H7 for the companion fix.

### H7 — `get_repo_state` hardcodes `limit=500`

**Files:** `src-tauri/src/lib.rs:1102, 1147`  
**Fix:** Add a `limit: Option<usize>` parameter to both Tauri commands:
```rust
#[tauri::command]
async fn get_repo_state(repo_path: String, limit: Option<usize>) -> Result<RepoStateResponse, String> {
    let limit = limit.unwrap_or(500);
    ...
    get_laned_commits_json(&rp, limit, 0)
    ...
}
```
Frontend `loadMoreCommits` already computes the growing limit; pass it through via `invoke('get_graph_state', { repoPath, limit: currentLength + 500 })`.

### H8 — `get_repo_state` fails entirely on partial sub-fetch error

**File:** `src-tauri/src/lib.rs:1101`  
**Fix:** Replace `tokio::join!` with independent fallible fetches that return defaults on failure:
```rust
let (commits_result, branches_result, tags_result, status_result) = tokio::join!(
    async { get_laned_commits_json(&rp, limit, 0).unwrap_or_default() },
    async { get_branches_json(&rp).unwrap_or_default() },
    async { get_tags_json(&rp).unwrap_or_default() },
    async { get_status_json(&rp).unwrap_or_default() },
);
```
Return `Ok(RepoStateResponse { commits: commits_result, ... })` even when some sub-fetches failed.

### H9 — `checkout_branch` blocks on unrelated untracked files

**File:** `src/branches/mod.rs:166`  
**Fix:** Remove the upfront status scan entirely. Call `repo.checkout_tree(...)` and let git2 return the natural `CheckoutConflict` error, then surface that message to the user directly. The user should see the actual git error, not a blanket "you have untracked files" block.

### H10 — `fetch` returns empty `updated_refs`

**File:** `src/remotes/mod.rs:139`  
**Fix:** Use `RemoteCallbacks::update_tips` to collect updated refs during the fetch:
```rust
let mut updated = vec![];
callbacks.update_tips(|refname, _old, _new| {
    updated.push(refname.to_string());
    true
});
```
Return `FetchResult { remote, updated_refs: updated, new_refs: [] }`.

### H11 — `push_branch` missing `--set-upstream`

**File:** `src/remotes/mod.rs:75`  
**Fix:** After successful push, configure tracking via git2:
```rust
repo.branch_set_upstream(branch_name, Some(&format!("{}/{}", remote_name, branch_name)))?;
```
This is equivalent to `git push -u origin <branch>`.

### H12 — `continue_rebase` silently skips `reword`

**File:** `src/rebase/mod.rs:185`  
**Fix:** Add `new_message: Option<String>` to `continue_rebase`. When provided, write the message to a temp file and set `GIT_EDITOR` to a shell script that copies it into the editor target (`$1`):
```rust
let editor_script = if let Some(msg) = new_message {
    let tmp = std::env::temp_dir().join("gitaxon_reword_msg");
    std::fs::write(&tmp, msg)?;
    format!("cp {} \"$1\"", tmp.display())
} else {
    "true".to_string()  // no-op editor for non-reword steps
};
Command::new(git_binary())
    .args(["rebase", "--continue"])
    .env("GIT_EDITOR", &editor_script)
    .env("GIT_SEQUENCE_EDITOR", "true")
    ...
```
Frontend RebasePanel: when `rebaseState.todo_items[0].action === 'reword'`, show an inline message textarea pre-filled with the current message. Pass the edited text via `invoke('continue_rebase', { repoPath, message: editedMsg })`.

### H13 — Interactive rebase uses short hashes in todo

**File:** `src/rebase/mod.rs:144`  
**Fix:** Change todo line format to use full hash:
```rust
format!("{} {} {}", item.action, item.hash, item.message)
// was: format!("{} {} {}", item.action, item.short_hash, item.message)
```

### H14 — Only one repo watched at a time

**File:** `src/watcher/mod.rs`  
**Fix:** Replace `static WATCHER: Mutex<Option<RecommendedWatcher>>` with `static WATCHERS: Mutex<HashMap<String, RecommendedWatcher>>`.  
`start_file_watch(repo_path)` inserts into the map by path.  
`stop_file_watch(repo_path)` removes by path, dropping the watcher.  
Each tab gets its own watcher; closing a tab triggers `stop_file_watch`.

### H15 — `.git/packed-refs` not watched

**File:** `src/watcher/mod.rs`  
**Fix:** Add explicit watch for `.git/packed-refs`:
```rust
watcher.watch(git_dir.join("packed-refs"), RecursiveMode::NonRecursive)?;
```
On `EventKind::Modify` or `Create` for this path, emit `git-state-changed`.

### H16 — Operation sentinel files not watched

**File:** `src/watcher/mod.rs`  
**Fix:** Watch these additional sentinel files for `Create`/`Remove` events:
- `.git/MERGE_HEAD` → emit `git-state-changed`
- `.git/CHERRY_PICK_HEAD` → emit `git-state-changed`
- `.git/REVERT_HEAD` → emit `git-state-changed`

Frontend: on `git-state-changed`, `store.ts` already calls `refreshStatus()` which loads `RepoOperationState` and shows the correct panel.

### H17 — Back button discards conflict resolver edits without warning

**File:** `ui/src/components/ConflictResolver.svelte`  
**Fix:** Track `isDirty`:
```ts
let isDirty = $derived(resolvedContent !== (file?.merged ?? ''));
```
On the back button click, if `isDirty`, show an inline "Discard changes?" confirmation with Cancel/Discard buttons. Only call `closeDiff()` on Discard.

### H18 — `todoItems` stale during rebase load

**File:** `ui/src/components/RebasePanel.svelte`  
**Fix:** Clear list before async fetch:
```ts
async function setupRebase(hash: string) {
    todoItems = [];       // ← add this line
    isWorking = true;
    try { ... }
    finally { isWorking = false; }
}
```

### H19 — `search_commits` returns isolated dots (no edges)

**File:** `src/graph/mod.rs:297`  
**Fix:** When building the commit list for lane assignment, include immediate parents of matching commits even if they don't match the search query:
```rust
let mut included = HashSet::new();
for c in &matching { included.insert(c.hash.clone()); }
for c in &matching {
    for parent in &c.parent_hashes {
        if !included.contains(parent) {
            if let Ok(pc) = get_commit_node(&repo, parent) {
                extended.push(pc);
                included.insert(parent.clone());
            }
        }
    }
}
```
Run `assign_lanes` on the extended set. Filter the display list back to matching commits only, but the lane/edge data now has valid parent rows.

### H20 — `stash_push` omits untracked files

**File:** `src/stash/mod.rs:57`  
**Fix:** Add `StashFlags::INCLUDE_UNTRACKED` to the stash save call:
```rust
repo.stash_save(&sig, &message, Some(git2::StashFlags::INCLUDE_UNTRACKED))?;
```

### H21 — `create_commit` doesn't emit `git-state-changed`

**File:** `src-tauri/src/lib.rs:300`  
**Fix:** After `create_commit` succeeds, emit the event:
```rust
app.emit("git-state-changed", ()).ok();
```
(Same pattern as `stage_file` which already does this.)

### H22 — `checkout_branch` doesn't emit `git-state-changed`

**File:** `src-tauri/src/lib.rs:679`  
**Fix:** After successful `checkout_branch` call, emit:
```rust
app.emit("git-state-changed", ()).ok();
```

---

## Wave 2 — UX & State Polish

### M1 — `goHome()` doesn't clear tabs

**File:** `ui/src/lib/store.ts`  
**Fix:** Add tab cleanup to `goHome`:
```ts
export function goHome() {
    openTabsStore.set([]);
    activeTabIndexStore.set(0);
    currentRepo.set(null);
    // existing cleanup...
}
```

### M2 — Push from warning modal has no loading spinner

**File:** `ui/src/components/AppShell.svelte`  
**Fix:** In the "Push Anyway" button handler, wrap with loading state:
```ts
async function handleForcePush() {
    isLoading.set(true);
    try { await executePush(); }
    finally { isLoading.set(false); }
}
```
Bind button `onclick` to `handleForcePush`.

### M3 — `selectFileFromStaging` uses wrong placeholder status

**File:** `ui/src/lib/store.ts`  
**Fix:** Map status code to the correct display status:
```ts
function toDisplayStatus(s: string): DiffFile['status'] {
    if (s === 'A') return 'Added';
    if (s === 'D' || s === 'WD') return 'Deleted';
    return 'Modified';
}
// Use toDisplayStatus(entry.status) instead of hardcoded 'Modified'
```

### M4 — `handleDeleteTag` has no confirmation

**File:** `ui/src/components/BranchSidebar.svelte`  
**Fix:** Add confirmation before deletion:
```ts
async function handleDeleteTag(tag: TagInfo) {
    if (!confirm(`Delete tag "${tag.name}"? This cannot be undone.`)) return;
    await deleteTag($currentRepo!, tag.name);
}
```
Or wire to the same confirmation modal used for branch deletion.

### M5 — `window.open` opens URLs inside Tauri WebView

**File:** `ui/src/components/BranchSidebar.svelte`  
**Fix:** Use the Tauri opener plugin:
```ts
import { open } from '@tauri-apps/plugin-opener';
// Replace: window.open(url, '_blank')
// With:    await open(url)
```
Add `tauri-plugin-opener` to `Cargo.toml` and register it in `src-tauri/src/lib.rs` and `tauri.conf.json`.

### M6 — `handleCanvasClick` O(n) scan over all commits

**File:** `ui/src/components/CommitGraph.svelte`  
**Fix:** Compute the row index directly from mouse Y position:
```ts
function handleCanvasClick(e: MouseEvent) {
    const scrollTop = graphScrollWrapRef.scrollTop;
    const absoluteY = scrollTop + e.offsetY;
    const rowIndex = Math.floor((absoluteY - HEADER_HEIGHT) / ROW_HEIGHT);
    if (rowIndex < 0 || rowIndex >= commitList.length) return;
    selectCommit(commitList[rowIndex].commit.hash);
}
```
O(1) instead of O(n).

### M7 — Context menu stays open on scroll

**File:** `ui/src/components/CommitGraph.svelte`  
**Fix:** In the scroll handler:
```ts
function onScroll() {
    ctxMenu = null;  // ← add this
    scheduleGraphReload();
}
```

### M8 — Context menu clips at viewport edges

**File:** `ui/src/components/CommitGraph.svelte`  
**Fix:** Clamp position after computing:
```ts
const MENU_W = 200, MENU_H = 280;
ctxMenu = {
    x: Math.min(e.clientX, window.innerWidth - MENU_W),
    y: Math.min(e.clientY, window.innerHeight - MENU_H),
    hash,
    ...
};
```

### M9 — `bisectBadPending` survives repo switch

**File:** `ui/src/components/CommitGraph.svelte`  
**Fix:**
```ts
$effect(() => {
    if ($currentRepo) bisectBadPending = null;
});
```

### M10 — `commentsForFile` O(n²) per render

**File:** `ui/src/components/PrReviewPanel.svelte`  
**Fix:** Precompute a map:
```ts
const commentsByFile = $derived.by(() => {
    const m = new Map<string, ReviewComment[]>();
    for (const c of comments) {
        const arr = m.get(c.path) ?? [];
        arr.push(c);
        m.set(c.path, arr);
    }
    return m;
});
// Replace commentsForFile(file.filename) with (commentsByFile.get(file.filename) ?? [])
```

### M11 — Rebase drag-and-drop no visual feedback

**File:** `ui/src/components/RebasePanel.svelte`  
**Fix:** Track `dragOverIndex` and apply CSS:
```ts
let dragIndex: number | null = null;
let dragOverIndex: number | null = null;
```
Add classes: `class:dragging={i === dragIndex}` and `class:drop-target={i === dragOverIndex}`.  
CSS: `.dragging { opacity: 0.4; }` and `.drop-target { border-top: 2px solid var(--accent); }`.

### M12 — `exec` action missing from ACTIONS list

**File:** `ui/src/components/RebasePanel.svelte`  
**Fix:** Add to the array: `const ACTIONS = ['pick', 'squash', 'fixup', 'reword', 'edit', 'drop', 'exec']`.  
When `item.action === 'exec'`, show an additional text input for the shell command alongside the action select.

### M13 — Wrong `{#each}` key causes full DOM rebuild on reorder

**File:** `ui/src/components/RebasePanel.svelte`  
**Fix:** Change the key from `(item.short_hash + i)` to `(item.hash)`:
```svelte
{#each todoItems as item, i (item.hash)}
```

### M14 — `handleAbort` doesn't set `isWorking`

**File:** `ui/src/components/RebasePanel.svelte`  
**Fix:**
```ts
async function handleAbort() {
    isWorking = true;
    try { await abortRebase($currentRepo!); }
    catch(e) { showToast(String(e), 'error'); }
    finally { isWorking = false; }
}
```

### M15 — Tab bar hidden at 1 tab

**File:** `ui/src/components/TabBar.svelte`  
**Fix:** Remove the `{#if tabs.length > 1}` condition. Show the tab bar whenever there is at least one tab:
```svelte
{#if tabs.length > 0}
```

### M16 — Active tab close button always hidden

**File:** `ui/src/components/TabBar.svelte`  
**Fix:** Add CSS rule:
```css
.tab.active .tab-close { opacity: 1; }
```

### M17 — `rows={lineCount(...)}` fights `flex:1` in ConflictResolver

**File:** `ui/src/components/ConflictResolver.svelte`  
**Fix:** Remove `rows={lineCount(resolvedContent)}` from the result textarea. Let `flex: 1; height: 100%; resize: none;` from the parent `.pane-editor` fill the space.

### M18 — BASE pane missing from 3-way merge editor

**File:** `ui/src/components/ConflictResolver.svelte`  
**Fix:** The `ConflictFile` type already has a `base: string` field. Add a third read-only pane:
```svelte
<div class="pane">
    <div class="pane-label">Base (common ancestor)</div>
    <textarea class="pane-editor" readonly value={file?.base ?? ''} />
</div>
```
Arrange panes in a 3-column flex row: OURS | BASE | THEIRS, with RESULT below spanning full width.

### M19 — "Continue" available before all conflicts resolved

**File:** `ui/src/components/ConflictResolver.svelte`  
**Fix:** Track resolved set:
```ts
let resolvedPaths = $state(new Set<string>());
// When markResolved is called for a file:
resolvedPaths = new Set([...resolvedPaths, currentFilePath]);
// Derived:
const allResolved = $derived(conflictedFiles.every(f => resolvedPaths.has(f)));
```
Bind: `<button disabled={!allResolved} onclick={handleContinue}>Continue</button>`.

### M20 & M21 — File history: no pagination, misleading count

**File:** `ui/src/components/FileHistoryPanel.svelte`  
**Fix:** Add `hasMore` flag from backend (return up to 500 entries + a `has_more: bool` field).  
Virtualize the list using the same `@tanstack/svelte-virtual` pattern as `CommitGraph.svelte`.  
Update count badge: `{entries.length}{hasMore ? '+' : ''} commits`.

### M22 — Watcher depth limit misses deep structures

**File:** `src/watcher/mod.rs`  
**Fix:** Change workdir watch to fully recursive:
```rust
watcher.watch(&workdir, RecursiveMode::Recursive)?;
```
Keep `.git/` sub-path watches as non-recursive (they only need specific files). Filter out events from `.git/` in the workdir watch handler to avoid double-processing.

### M23 — `start_file_watch` accumulates dangling threads

**File:** `src-tauri/src/lib.rs:72`  
**Fix:** After H14 (per-repo watcher HashMap), this is naturally solved — the old watcher is dropped from the map when a new one is inserted for the same key. No dangling thread issue because the thread is owned by the watcher struct.

### M24 — PR detection only checks `origin` remote

**File:** `src-tauri/src/lib.rs:904`  
**Fix:** Walk all remotes and pick the GitHub/GitLab one:
```rust
fn find_github_remote(repo: &Repository) -> Option<(String, String)> {
    for name in repo.remotes().ok()?.iter().flatten() {
        if let Ok(remote) = repo.find_remote(name) {
            let url = remote.url().unwrap_or("");
            if url.contains("github.com") || url.contains("gitlab.com") {
                return Some((name.to_string(), url.to_string()));
            }
        }
    }
    None
}
```

### M25 — No token returns empty string instead of error

**File:** `src-tauri/src/lib.rs:916`  
**Fix:** Return `Option<String>`:
```rust
fn get_token(platform: &str) -> Option<String> {
    keyring::Entry::new("gitaxon", platform).ok()?.get_password().ok()
}
```
In callers (`list_prs`, `list_issues`, etc.), if `get_token` returns `None`, return a structured error: `"No API token configured. Add one in Settings → Tokens."`. Frontend shows this as a toast with a link to settings.

### M26 — `GIT_SEQUENCE_EDITOR` uses `cp` (Unix-only)

**File:** `src/rebase/mod.rs:159`  
**Fix:** Write the todo file directly in Rust — no shell command needed:
```rust
let todo_path = repo_path.join(".git/rebase-merge/git-rebase-todo");
std::fs::write(&todo_path, todo_content)?;
// Remove the GIT_SEQUENCE_EDITOR=cp env var entirely
```

### M27 — Non-interactive rebase progress shows 0/0

**File:** `src/rebase/mod.rs:43`  
**Fix:** Read progress from `rebase-apply` directory:
```rust
let next_file = git_dir.join("rebase-apply/next");
let last_file = git_dir.join("rebase-apply/last");
let current_step = std::fs::read_to_string(&next_file)
    .ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
let total_steps = std::fs::read_to_string(&last_file)
    .ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
```

### M28 — `git bisect visualize` can invoke GUI

**File:** `src/bisect/mod.rs:63`  
**Fix:** Parse `.git/BISECT_LOG` to count remaining steps:
```rust
let log = std::fs::read_to_string(git_dir.join("BISECT_LOG")).unwrap_or_default();
let remaining = log.lines()
    .filter(|l| l.starts_with("# ") && !l.starts_with("# bad:") && !l.starts_with("# good:"))
    .count();
```
Or use `git log HEAD...BISECT_BAD --oneline | wc -l` if bisect log parsing is unreliable.

### M29 — Blame parser ignores `filename` for renamed files

**File:** `src/diff/mod.rs:486`  
**Fix:** In the porcelain blame parser, capture the `filename` header:
```rust
"filename" => { current_entry.filename = Some(value.to_string()); }
```
Add `filename: Option<String>` to the `BlameLine` struct. Frontend shows the filename on hover for lines from renamed files.

### M30 — Tags sorted alphabetically, not by date

**File:** `src/tags/mod.rs:91`  
**Fix:**
```rust
tags.sort_by(|a, b| b.tagger_date.cmp(&a.tagger_date));
```
For lightweight tags (no `tagger_date`), fall back to the commit date by peeling the tag to its commit.

### M31 — `list_tags` opens fresh Repository handle

**File:** `src/tags/mod.rs:23`  
**Fix:** Accept a `&Repository` reference as a parameter rather than `repo_path: &str`. Update all callers to pass the pooled `repo` handle.

### M32 — Dual library overhead: gix + git2

**File:** `src/graph/mod.rs`  
**Fix:** Consolidate on `gix` for all graph operations (it's already the primary rev-walk library). Replace the `git2::Repository` usage in `collect_all_ref_tips` with gix ref iteration:
```rust
let repo = gix::open(&repo_path)?;
for reference in repo.references()?.all()? {
    let r = reference?;
    if let Ok(id) = r.peel_to_id_in_place() {
        tips.push(id.to_owned());
    }
}
```

### M33 — `git` subprocess uses relative path (unreliable on macOS Tauri)

**File:** `src/remotes/mod.rs`  
**Fix:** Add a git binary resolver:
```rust
static GIT_BIN: OnceLock<PathBuf> = OnceLock::new();

fn git_binary() -> &'static PathBuf {
    GIT_BIN.get_or_init(|| {
        for path in ["/usr/bin/git", "/usr/local/bin/git", "/opt/homebrew/bin/git"] {
            let p = PathBuf::from(path);
            if p.exists() { return p; }
        }
        PathBuf::from("git")  // fallback
    })
}
```
Replace `Command::new("git")` with `Command::new(git_binary())` throughout `remotes/mod.rs` and `rebase/mod.rs`.

### M34 — `loadRepo` silently drops queued navigation

**File:** `ui/src/lib/store.ts`  
**Fix:** Queue the pending path and handle it after load completes:
```ts
let pendingRepo: string | null = null;

async function loadRepo(path: string) {
    if (loadRepoLock) {
        pendingRepo = path;
        return;
    }
    loadRepoLock = true;
    try { await doLoadRepo(path); }
    finally {
        loadRepoLock = false;
        if (pendingRepo) {
            const next = pendingRepo;
            pendingRepo = null;
            await loadRepo(next);
        }
    }
}
```

---

### L1 — Dead `handleGraphWheel` function

**File:** `ui/src/components/CommitGraph.svelte`  
**Fix:** Delete the `handleGraphWheel` function entirely.

### L2 — Dead `label.isTag` guard on `BranchInfo`

**File:** `ui/src/components/CommitGraph.svelte`  
**Fix:** Remove the `if (label.isTag)` branch from `handleBranchDoubleClick`. Tags don't go through this handler.

### L3 — Stash rows not keyboard accessible

**File:** `ui/src/components/StashManager.svelte`  
**Fix:** Change stash row elements from `<div>` to `<button>`:
```svelte
<button class="stash-item" onclick={() => handleSelect(stash)}>
```
Buttons are natively keyboard-focusable and activate on Enter/Space.

### L4 — `stashPop` with no dirty-tree warning

**File:** `ui/src/components/StashManager.svelte`  
**Fix:** Before popping, check if status store has entries:
```ts
async function handlePop(stash: StashEntry) {
    if ($statusEntries.length > 0) {
        if (!confirm('You have uncommitted changes. Popping may cause conflicts. Continue?')) return;
    }
    await stashPop($currentRepo!, stash.index);
}
```

### L5 — Worktree right-click directly triggers remove

**File:** `ui/src/components/BranchSidebar.svelte`  
**Fix:** Implement a proper context menu for worktree items:
```svelte
oncontextmenu={(e) => { e.preventDefault(); showWorktreeMenu(e, worktree); }}
```
Menu options: "Switch to Worktree", "Open in Terminal", "Remove Worktree". Only "Remove" triggers the deletion prompt.

### L6 — `cursor: grab` shown in progress mode

**File:** `ui/src/components/RebasePanel.svelte`  
**Fix:**
```css
.rb-item { cursor: grab; }
.rb-item.no-drag { cursor: default; }
```
```svelte
class:no-drag={mode !== 'setup'}
```

### L7 — PR review submit allows empty comment body

**File:** `ui/src/components/PrReviewPanel.svelte`  
**Fix:**
```ts
async function handleSubmitReview(event: string) {
    if (event === 'COMMENT' && !reviewBody.trim()) {
        showToast('Comment body cannot be empty', 'error');
        return;
    }
    ...
}
```

### L8 — Async subscribe callback swallows errors

**File:** `ui/src/components/PrReviewPanel.svelte`  
**Fix:**
```ts
const unsub = selectedPr.subscribe(async (num) => {
    try {
        if (num !== null) await loadPrData(num);
    } catch (e) {
        showToast(String(e), 'error');
    }
});
```

### L9 — Binary detection samples only 8 KB

**File:** `src/diff/mod.rs:317`  
**Fix:** Increase sample to 64 KB for large files:
```rust
const BINARY_SAMPLE: usize = 65_536;
let sample = &content[..content.len().min(BINARY_SAMPLE)];
sample.contains(&0u8)
```

### L10 — `parse_unified_diff` doesn't update `old_path` from `rename from`

**File:** `src/diff/mod.rs:644`  
**Fix:**
```rust
} else if line.starts_with("rename from ") {
    current_file.old_path = Some(line["rename from ".len()..].to_string());
} else if line.starts_with("rename to ") {
    current_file.new_path = Some(line["rename to ".len()..].to_string());
}
```

### L11 — Test commits have `timestamp: 0` (non-deterministic sort)

**File:** `src/graph/lanes.rs`  
**Fix:** Assign monotonically increasing timestamps in tests:
```rust
CommitNode { hash: "aaa", timestamp: 1000, ... }
CommitNode { hash: "bbb", timestamp: 2000, ... }
```

### L12 — `search_commits` author filter not sanitized

**File:** `src/graph/mod.rs:297`  
**Fix:**
```rust
if author.contains('\n') || author.contains('\0') || author.contains('\r') {
    return Err("Invalid characters in author filter".into());
}
```

### L13 — Watcher errors swallowed silently

**File:** `src-tauri/src/lib.rs:88`  
**Fix:**
```rust
Err(e) => {
    eprintln!("[Watcher] Error: {e}");
    app.emit("watcher-error", e.to_string()).ok();
}
```
Frontend: listen for `watcher-error` event and show a dismissible warning banner in AppShell.

### L14 — Bisect state not cleaned up on repo switch

**File:** `ui/src/lib/store.ts`  
**Fix:** In `loadRepo` (before loading the new repo), if bisect is active, call reset:
```ts
if (get(bisectStateStore)?.active) {
    await resetBisect(get(currentRepo)!);
}
```

### L15 — Stash index may not match `stash@{N}`

**File:** `src/stash/mod.rs`  
**Fix:** Parse the stash index from the `%gd` field (already in the format string, e.g., `stash@{0}`):
```rust
let index: usize = gd.trim_start_matches("stash@{")
                      .trim_end_matches('}')
                      .parse().unwrap_or(i);
```

### L16 — `buildHunkPatch` missing `\ No newline at end of file` marker

**File:** `ui/src/components/DiffViewer.svelte`  
**Fix:** After the last line of each hunk block, check if the final context/added line ends without `\n`:
```ts
if (lastLine && !lastLine.content.endsWith('\n')) {
    patch += '\\ No newline at end of file\n';
}
```

### L17 — Undo/Redo are dead interactive buttons

**File:** `ui/src/components/AppShell.svelte`  
**Fix:** Either remove them from the toolbar, or disable with tooltip:
```svelte
<button disabled title="Coming soon">Undo</button>
<button disabled title="Coming soon">Redo</button>
```

---

## Wave 3 — Performance & Architecture

### P1 — `hashToRow` rebuilt on every canvas draw tick

**File:** `ui/src/components/CommitGraph.svelte`  
**Fix:** Move `hashToRow` to a `$derived` computed once when `commitList` changes:
```ts
const hashToRow = $derived.by(() => {
    const m = new Map<string, number>();
    commitList.forEach((c, i) => m.set(c.commit.hash, i));
    return m;
});
```
Remove it from the `drawVisibleGraph()` call path entirely.

### P2 — Full graph reload after every push/fetch

**File:** `ui/src/lib/store.ts`  
**Fix:** After push/fetch, reload only the portion beyond the current tip:
```ts
async function incrementalGraphUpdate() {
    const currentLen = get(commitsStore).length;
    const updated = await invoke('get_graph_state', { 
        repoPath: get(currentRepo), 
        limit: currentLen + 50  // fetch just a bit beyond current
    });
    // Merge updated commits into store rather than replacing
}
```
Trigger `incrementalGraphUpdate()` instead of full `scheduleGraphReload()` after push/fetch.

### P3 — Dual gix/git2 overhead

Already specified in M32.

### P4 — Fresh Repository handle in `list_tags`

Already specified in M31.

### P5 — Per-page independent lane sort

Already specified in H6 + H7.

### P6 — O(n) canvas click scan

Already specified in M6.

---

## Wave 4 — Enterprise Features

### E1 — GPG/SSH commit signing

**New:** `src/signing/mod.rs`  
Read `user.signingkey` and `commit.gpgsign` from `repo.config()`. When `gpgsign=true`, pipe the commit object through `gpg --armor --detach-sign` and set the `gpgsig` header. For SSH signing, use `ssh-keygen -Y sign`. Add "Sign commits" checkbox to the commit box in `RightPanel.svelte`. Show signing status badge on signed commits in the graph tooltip.

### E2 — Custom CA / certificate pinning

**File:** `src/remotes/mod.rs`  
Read `http.sslCAInfo` from git config. Pass to `reqwest::ClientBuilder::add_root_certificate()` for all API calls. Add `http.sslVerify=false` detection and show a security warning in the UI if set.

### E3 — HTTPS proxy support

**File:** `src/remotes/mod.rs`  
Read `http.proxy` and `https.proxy` from git config. Apply to `reqwest::ClientBuilder::proxy()` for all API calls. Also pass as `GIT_HTTP_PROXY` env var to git subprocess calls.

### E4 & E5 — OAuth flow for GitHub/GitLab

**New:** `src-tauri/src/oauth.rs` + Tauri deep link handler.  
- Register `gitaxon://` URI scheme in `tauri.conf.json`.
- `initiate_oauth(platform)`: open browser to `github.com/login/oauth/authorize?...`.
- Deep link handler: receive `gitaxon://oauth/callback?code=...`, exchange for token, store in keychain.
- Frontend: "Connect GitHub" button in Settings → Tokens page.

### E6 — Operation audit log

**New:** `src/audit/mod.rs`  
Append-only log at `~/.gitaxon/audit.log`. Log format: `[ISO8601] [repo] [operation] [result]`. Operations: push, pull, merge, rebase, cherry-pick, reset, stash.  
New `AuditPanel.svelte` accessible from a "History" icon in the sidebar. Shows last 200 operations with timestamps and repo names.

### E7 — Co-author support in commit box

**File:** `ui/src/components/RightPanel.svelte`  
Add a collapsible "Add co-authors" section below the commit message. Each entry: name + email. Appends `Co-authored-by: Name <email>` trailers to the commit message before calling `create_commit`.

### E8 — Date range filter in graph view

The `filterSince`/`filterUntil` stores already exist in `store.ts`. Wire them to a date-range picker in the `CommitGraph.svelte` toolbar. Pass through to the `get_graph_state` Tauri command as `since: Option<i64>` / `until: Option<i64>` Unix timestamps. Rust filters commits before lane assignment.

### E9 — Side-by-side diff mode

**File:** `ui/src/components/DiffViewer.svelte`  
Add a view toggle: Unified | Split. In split mode, render a two-column table:

| Old line # | Old content | New line # | New content |
|---|---|---|---|

Deleted lines show in left column (red), added lines show in right column (green), context lines appear in both. Use synchronized scrolling (both columns scroll together via a shared scroll handler).

### E10 — Syntax highlighting in diff

**New dep:** `@speed-highlight/core` (7 KB, tree-shakeable, no highlight.js weight).  
In `DiffViewer.svelte`, detect language from file extension. Apply highlighting to `.line-content` cells in the diff table. Only highlight context and added lines (deleted lines are removed content — highlighting is less critical). Highlighting runs in a `$effect` after diff renders.

### E11 — Image diff

**File:** `ui/src/components/DiffViewer.svelte`  
For image file extensions (`.png`, `.jpg`, `.jpeg`, `.gif`, `.svg`, `.webp`): skip the text diff table. Instead, show old and new images side by side. Add an opacity slider (0-100%) to overlay new image on top of old for a "difference" view.

### E12 — BASE pane in conflict resolver

Already specified in M18.

### E13 — PR creation form

**New:** `ui/src/components/CreatePrPanel.svelte`  
Uses existing `createPr` IPC command. Fields: title (required), body (markdown textarea), base branch (dropdown of remote branches), draft toggle.  
Accessible from a "New PR" button in `BranchSidebar.svelte` PR section header. On success, refresh the PR list and navigate to the new PR.

### E14 — Inline PR comment creation

**File:** `ui/src/components/PrReviewPanel.svelte`  
Add a `+` hover button on each diff line in the PR file diff pane. Clicking expands an inline comment form (textarea + Submit button). New Tauri command `add_pr_comment(repo_path, pr_number, path, line, body)` calls `POST /repos/{owner}/{repo}/pulls/{pr}/comments`.

### E15 — PR merge button

**File:** `ui/src/components/PrReviewPanel.svelte`  
Add a Merge dropdown button with three options: Merge commit, Squash and merge, Rebase and merge. New Tauri command `merge_pr(repo_path, pr_number, method)` calls `PUT /repos/{owner}/{repo}/pulls/{pr}/merge`. Refresh PR list on success.

### E16 — Notification system

**New:** `src-tauri/src/notifications.rs` + `ui/src/components/NotificationPanel.svelte`  
Poll `GET /notifications` every 60 seconds when a GitHub token is configured. Show unread count badge on the bell icon in `AppShell.svelte`. Click opens a slide-out panel with notification list. Mark as read on click.

### E17 — Settings panel

**New:** `ui/src/components/SettingsPanel.svelte`  
Sections: Git Identity (name/email editor), Tokens (GitHub/GitLab token input with Connect via OAuth button), Appearance (theme selector, font size), Git Config (key-value editor for common settings), Proxy, Signing.  
Wire the gear icon in `AppShell.svelte` to navigate to this panel (new `centerView = 'settings'` or a drawer overlay).

### E18 — Terminal launch on Linux/Windows

**File:** `src-tauri/src/lib.rs:1004`  
Linux: try in order: `gnome-terminal`, `konsole`, `xfce4-terminal`, `lxterminal`, `xterm`.  
Windows: try `wt.exe -d .` (Windows Terminal), then `cmd.exe /K cd /d .`.  
Wrap in `#[cfg(target_os = "linux")]` / `#[cfg(target_os = "windows")]` blocks.

### E19 — Git binary and cross-platform paths

**File:** `src/remotes/mod.rs` and all subprocess callers.  
Already specified in M33. Additionally: normalize all path separators in IPC responses for Windows compatibility (`path.replace('\\', '/')` for display, preserve native for filesystem calls).

### E20 & E21 — Incremental graph update / hashToRow performance

Already specified in P1 and P2.

### E22 — Keyboard navigation in commit graph

**File:** `ui/src/components/CommitGraph.svelte`  
Add `tabindex="0"` to the `graphScrollWrapRef` container.  
Handle `keydown` events:
- `ArrowDown`: select next commit
- `ArrowUp`: select previous commit
- `Enter`: expand commit detail panel
- `Escape`: deselect / close context menu
- `c`: cherry-pick selected commit
- `b`: start bisect on selected commit

### E23 — Screen reader labels on canvas

**File:** `ui/src/components/CommitGraph.svelte`  
Add `role="application"` and `aria-label="Commit graph"` to the scroll container.  
Add a visually-hidden live region:
```svelte
<div aria-live="polite" class="sr-only">
    {#if $selectedCommit}
        Commit {$selectedCommit.short_hash}: {$selectedCommit.message} by {$selectedCommit.author_name}
    {/if}
</div>
```

### E24 — i18n infrastructure

**New dep:** `@inlang/paraglide-js` (compile-time tree-shaking, no runtime overhead).  
Extract all hardcoded UI strings to `messages/en.json`. Replace `"Push"` etc. with `m.push()`. Ships English only; infrastructure in place for community contributions.

### E25 & E26 — Test coverage

**Frontend:** Add `vitest` + `@testing-library/svelte` for store unit tests. Priority: `store.ts` functions (`loadRepo`, `selectCommit`, `refreshStatus`), `ConflictResolver` resolution flow, `PrReviewPanel` race condition fix.  
**Rust:** Add `#[cfg(test)]` modules in each `mod.rs` using a `tempdir()` git repo fixture. Priority: `staging/mod.rs` (initial commit edge cases), `branches/mod.rs` (merge/delete guards), `diff/mod.rs` (binary detection, blame parser).  
**E2E:** Add `playwright` for critical path: open repo → stage file → commit → check graph updates.

---

## Data Flow Invariants (unchanged by these fixes)

- All state flows one direction: Rust backend → Tauri IPC → Svelte stores → components
- Mutation flows: Component action → `invoke()` → Rust command → emit event → store refresh
- No component directly mutates global state — all mutations go through `store.ts` exports
- File watcher events are the only push path; all other updates are pull

## Non-goals

- No new IPC command schemas (all new commands follow existing `snake_case` naming and `Result<T, String>` return type)
- No changes to the lane assignment algorithm (covered separately in the graph quality spec)
- No migration to a different build system
- No UI framework change
