# GitAxon

A fast, native Git GUI built with Tauri 2, SvelteKit, and Rust. GitAxon aims to deliver GitKraken-quality commit graph visualization with near-zero startup latency using `gix` for all Git operations.

---

## Features

- **Commit graph** — GitKraken-style lane visualization with continuous branch lines, consistent branch colors, and S-curve Bezier arcs across scroll boundaries
- **Branch management** — list, create, switch, and delete local and remote branches
- **Staging area** — interactive file staging/unstaging with line-level diff preview
- **Diff viewer** — side-by-side and unified diffs for working tree, staged changes, and any commit
- **Conflict resolver** — visual three-way merge conflict resolution
- **Rebase** — interactive rebase panel with drag-to-reorder
- **Stash manager** — create, apply, and drop stash entries
- **File history** — per-file commit timeline
- **Worktree support** — create and switch between Git worktrees
- **Tags** — list and manage local tags
- **Remote operations** — fetch, push, pull with remote management
- **PR review panel** — view pull request diffs inline (GitHub integration)
- **File watcher** — automatic UI refresh on filesystem changes via `notify`

---

## Architecture

```
GitAxon/
├── src/                        # Rust library crate (gitaxon)
│   ├── lib.rs                  # Module exports and public API
│   ├── main.rs                 # CLI entry point (gitfast)
│   ├── graph/                  # Commit graph engine
│   │   ├── mod.rs              # gix walk → topological sort → JSON output
│   │   └── lanes.rs            # Lane assignment + through_lanes sweep-line
│   ├── branches/               # Branch CRUD
│   ├── diff/                   # Diff, blame, file status
│   ├── staging/                # Index (stage/unstage)
│   ├── history/                # Commit log
│   ├── rebase/                 # Interactive rebase
│   ├── conflicts/              # Merge conflict detection
│   ├── stash/                  # Stash operations
│   ├── tags/                   # Tag management
│   ├── remotes/                # Fetch/push/pull
│   ├── worktree/               # Git worktrees
│   ├── bisect/                 # git bisect
│   ├── watcher/                # Filesystem watcher (notify)
│   ├── cache/                  # In-process commit cache
│   ├── repo_pool/              # Repository handle pool
│   ├── identity/               # User identity (author/committer)
│   └── errors.rs               # Typed error types
│
├── src-tauri/                  # Tauri shell
│   └── src/
│       ├── lib.rs              # Tauri commands (IPC bridge → Rust library)
│       ├── github.rs           # GitHub API integration
│       └── main.rs             # Tauri app entry point
│
└── ui/                         # SvelteKit frontend
    └── src/
        ├── components/
        │   ├── CommitGraph.svelte      # Canvas graph renderer (TanStack Virtual)
        │   ├── AppShell.svelte         # Root layout, tab management
        │   ├── BranchSidebar.svelte    # Branch/tag/remote tree
        │   ├── DiffViewer.svelte       # Unified/split diff display
        │   ├── RightPanel.svelte       # Commit detail + file list
        │   ├── ConflictResolver.svelte # Three-way merge UI
        │   ├── RebasePanel.svelte      # Interactive rebase UI
        │   ├── StashManager.svelte     # Stash list and actions
        │   ├── FileHistoryPanel.svelte # Per-file history
        │   ├── PrReviewPanel.svelte    # PR review diff viewer
        │   ├── CenterPanel.svelte      # Center content router
        │   ├── TabBar.svelte           # Repo tab bar
        │   ├── Toast.svelte            # Notification toasts
        │   └── Welcome.svelte          # Repo picker / onboarding
        └── lib/
            ├── store.ts        # Svelte stores + IPC call wrappers
            ├── tauri.ts        # Raw Tauri invoke() bindings
            ├── types.ts        # Shared TypeScript types
            └── toast.ts        # Toast notification helpers
```

### Data pipeline

```
gix walk (Rust)
  → temporal_topological_sort()       # DFS post-order, newest-first
  → assign_lanes()                    # GitKraken forbidden-column algorithm
      → through_lanes sweep-line      # Emits active-lane bitmask per row
  → generate_edges()                  # Parent→child edge list with EdgeType
  → JSON via serde_json
  → Tauri IPC (invoke)
  → commitsStore (Svelte writable)    # Growing-window replace on load-more
  → CommitGraph.svelte (Canvas API)   # TanStack Virtualizer + DPR-scaled canvas
```

---

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop shell | Tauri 2 |
| Frontend | SvelteKit 5 + TypeScript |
| Build | Vite 7 + Bun |
| Virtualization | TanStack Virtual Core |
| Git backend | gix 0.81 + git2 0.20 |
| Serialization | serde + serde_json |
| Async runtime | Tokio |
| Error handling | thiserror + anyhow |
| Filesystem events | notify + notify-debouncer-mini |
| Storage | rusqlite (bundled) |
| CLI parsing | clap 4 |

---

## Prerequisites

- **Rust** — stable toolchain (`rustup update stable`)
- **Node.js** 18+ and **Bun** (`npm install -g bun`)
- **Tauri CLI** dependencies — see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform

On macOS, you also need Xcode Command Line Tools:

```bash
xcode-select --install
```

---

## Getting Started

### Development

```bash
# 1. Install frontend dependencies
cd ui && bun install && cd ..

# 2. Run in development mode (hot-reload frontend + Rust backend)
cd ui && bun run tauri dev
```

The app window opens automatically. The frontend hot-reloads on Svelte changes; Rust changes trigger a recompile.

### Build for production

```bash
cd ui && bun run tauri build
```

Outputs a `.dmg` and `.app` bundle in `src-tauri/target/release/bundle/`.

### CLI (optional)

The `gitfast` CLI exposes the Rust library directly:

```bash
cargo build --release

# Show commit graph
./target/release/gitfast graph --repo /path/to/repo --limit 200

# Show commit log
./target/release/gitfast log --repo /path/to/repo

# Show diff for a commit
./target/release/gitfast diff --repo /path/to/repo --commit <sha>

# Stage all changes
./target/release/gitfast stage --repo /path/to/repo --all

# List branches
./target/release/gitfast branches --repo /path/to/repo
```

---

## Commit Graph Implementation

The graph engine (`src/graph/`) implements the GitKraken-style forbidden-column lane assignment algorithm:

1. **Topological sort** — `temporal_topological_sort()` performs DFS post-order over the `gix` commit walk, producing a globally stable newest-first ordering. This runs on the full accumulated commit set, not per-page — ensuring lane N on rows 1–500 is the same lane N on rows 501–1000.

2. **Lane assignment** — `assign_lanes()` iterates top-to-bottom. Each commit picks the leftmost column not forbidden by any active branch. A column is forbidden if a sibling branch is passing through it (the GitKraken algorithm).

3. **Through-lanes** — a sweep-line pass emits a `through_lanes: Vec<usize>` per row — the set of lane indices that have an active branch passing through but no commit on that row. The Canvas renderer uses this to draw continuous vertical lines without re-deriving lane state from the edge list.

4. **Branch color continuity** — colors follow branch lineage, not column index. A tip commit gets the next color from a global counter; continuation commits inherit the color from their child (the commit that came before on the same branch). This keeps a branch the same color even if it shifts lanes due to merges.

5. **Growing-window pagination** — `loadMoreCommits()` in `store.ts` calls `getCommits(repo, currentLength + 500, offset=0)` and replaces the store. The Rust side processes the full accumulated history in one pass, so edges and lanes are globally consistent at every page boundary.

6. **Edge rendering** — `CommitGraph.svelte` renders edges as cubic Bezier S-curves on a DPR-scaled `<canvas>`. Merge edges (many parents → one child) use the vscode-git-graph arc algorithm: the curve starts at the source commit's y-center and ends at the target commit's y-center, with control points at `d = ROW_HEIGHT * 0.8`.

---

## Project Structure — Rust Modules

| Module | Purpose |
|---|---|
| `graph` | Commit graph: walk, sort, lane assignment, edge generation |
| `branches` | List/create/delete/switch branches |
| `diff` | File diffs, blame, working-tree vs index vs commit |
| `staging` | Stage and unstage files, read index status |
| `history` | Commit log queries |
| `rebase` | Interactive rebase (edit, squash, fixup, reorder) |
| `conflicts` | Detect and parse merge conflicts |
| `stash` | Create, list, apply, drop stash entries |
| `tags` | List, create, delete tags |
| `remotes` | Fetch, push, pull, remote CRUD |
| `worktree` | List, create, remove Git worktrees |
| `bisect` | Start, mark good/bad, reset bisect session |
| `watcher` | Debounced filesystem watcher for auto-refresh |
| `cache` | In-memory commit node cache keyed by SHA |
| `repo_pool` | Pooled `gix::Repository` handles to avoid repeated open |
| `identity` | Read and write author/committer config |
| `errors` | `GitfastError` + `GitfastResult` type aliases |

---

## Contributing

1. Fork the repo and create a feature branch off `development`
2. Run `cargo test` before submitting — graph unit tests live in `src/graph/lanes.rs`
3. For UI changes, start `bun run tauri dev` and test with a real multi-branch repository
4. Open a PR against `development`

---

## License

MIT
