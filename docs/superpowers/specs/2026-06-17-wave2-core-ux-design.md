# Wave 2 — Core UX Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Bring GitAxon's daily-use UX to a professional level — keyboard shortcuts, side-by-side diff, git blame, virtualized commit list, and onboarding improvements.

**Architecture:** All changes are additive — no existing APIs change. New UI modes are toggled via Svelte `$state` without touching the Rust backend except for the blame command. The virtual scroller wraps the existing commit row renderer.

**Tech Stack:** SvelteKit 5 runes · Tauri 2 · Rust `git2` · `@tanstack/virtual` (already in deps)

---

## Feature 1 — Keyboard Shortcuts

### What
Global keyboard shortcuts bound at the `AppShell` level via `svelte:window on:keydown`. A `⌘K` command palette (searchable list) sits on top.

### Bindings
| Shortcut | Action |
|---|---|
| `⌘Enter` | Commit staged changes |
| `⌘S` | Stage / unstage selected file |
| `⌘Z` | Discard selected file |
| `⌘P` | Push |
| `⌘F` | Fetch |
| `⌘K` | Open command palette |
| `Escape` | Close palette / cancel modal |
| `⌘⇧Z` | Undo last commit (amend) |

### Components
- `ui/src/components/CommandPalette.svelte` — new. Floating overlay, fuzzy-filter list of all actions, keyboard-navigable.
- `ui/src/components/AppShell.svelte` — add global `keydown` handler, render `<CommandPalette />`.

### Data flow
`AppShell` captures `keydown` → calls existing store functions (`commitChanges`, `pushBranch`, etc.) directly. Palette renders a static `ACTIONS` array and calls the same functions on selection.

---

## Feature 2 — Side-by-Side Diff View

### What
A toggle button in `DiffViewer`'s toolbar switches between **Unified** (current) and **Split** view. Split view shows old content on the left, new on the right, scroll-synced.

### Components
- `ui/src/components/DiffViewer.svelte` — add `splitMode: boolean` state, render split layout when true.
- No Rust changes needed — same `DiffFile`/`DiffHunk`/`DiffLine` structs.

### Split layout
Two `<div class="pane">` side-by-side inside a `<div class="split-view">`. Left pane renders `Deleted` + `Context` lines; right pane renders `Added` + `Context` lines. `scroll` event on either pane syncs `scrollTop` of the other.

---

## Feature 3 — Git Blame

### What
A **Blame** tab in the diff/file viewer toolbar. Shows each line annotated with author, short hash, and relative date in a left gutter. Clicking a line's hash navigates to that commit in the graph.

### Rust backend — new command
`src/blame/mod.rs` — new module.

```rust
pub struct BlameHunk {
    pub commit_hash: String,
    pub author: String,
    pub author_time: i64,   // unix timestamp
    pub line_start: u32,
    pub line_count: u32,
}

pub struct BlameResult {
    pub hunks: Vec<BlameHunk>,
    pub lines: Vec<String>,  // raw file lines
}

pub async fn blame_file(repo_path: &str, file_path: &str, commit: Option<&str>) -> GitfastResult<BlameResult>
```

Uses `git2::Repository::blame_file()`. Exposes as Tauri command `get_blame`.

### Frontend
- `ui/src/lib/tauri.ts` — add `getBlame(repoPath, filePath, commit?)`.
- `ui/src/components/DiffViewer.svelte` — add `Blame` tab alongside `Diff View` / `File View`. When active, renders `BlameViewer` inline.
- `ui/src/components/BlameViewer.svelte` — new. Renders lines with gutter showing hash + author + date. Click on hash calls `selectCommitByHash(hash)` from store.

---

## Feature 4 — Virtualized Commit List

### What
Replace the full DOM render of all commit rows in `CommitGraph.svelte` with a virtual window — only the ~30 visible rows render at any time. Fixes lag on repos with 5,000+ commits.

### Approach
`@tanstack/virtual` is already in `package.json`. Use `useVirtualizer` (or the Svelte equivalent `createVirtualizer`) with:
- `count` = total commits
- `getScrollElement` = the graph scroll container ref
- `estimateSize` = `ROW_HEIGHT` (constant already defined in CommitGraph)

The canvas lane-drawing layer stays unchanged — it already renders by pixel range. Only the DOM row list switches to virtual rendering.

### Components
- `ui/src/components/CommitGraph.svelte` — wrap commit row list with virtualizer. Keep canvas layer as-is.

---

## Feature 5 — Onboarding / Welcome Screen

### What
Improve `Welcome.svelte`:
1. **Recent repos list** — stored in `tauri-plugin-store`, shown as clickable cards
2. **Clone repo button** — opens a dialog asking for URL + destination path, calls `git clone` via a new Rust command
3. **What's new section** — hardcoded list of latest features (updated each release)

### Rust backend — new command
`src/remotes/mod.rs` — add `clone_repo(url: &str, dest: &str)` using `git2::build::RepoBuilder`.

### Frontend
- `ui/src/components/Welcome.svelte` — redesign with three sections: Recent Repos, Clone, What's New.
- `ui/src/lib/store.ts` — persist `recentRepos` to `tauri-plugin-store` on every `loadRepo()` call, read on startup.
