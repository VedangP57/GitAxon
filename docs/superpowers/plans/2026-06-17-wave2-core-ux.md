# Wave 2 — Core UX Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add keyboard shortcuts + command palette, side-by-side diff view, git blame viewer, and wire up the Welcome screen's clone button.

**Architecture:** All frontend-only except the clone Rust command. Virtualizer and blame Rust backend already exist — tasks focus on missing UI. No existing store APIs change; new actions call existing tauri.ts functions.

**Tech Stack:** SvelteKit 5 runes · Tauri 2 · Rust git2 · `@tanstack/virtual-core` (already in deps)

**Global Constraints:**
- SvelteKit 5 runes only: `$state`, `$derived`, `$effect` — no legacy `writable`/`onMount`
- No new Rust commands except `clone_repo`
- Existing store functions: `fetchFromRemote`, `loadRepo` (store.ts); `createCommit`, `push`, `fetchRemote`, `gitBlame` (tauri.ts)
- All components use CSS vars: `--bg-primary`, `--bg-secondary`, `--border`, `--text-primary`, `--text-muted`, `--accent-blue`, `--accent-green`, `--accent-red`
- `cargo build` and `npx tsc --noEmit` must stay clean after every task

---

## File Map

| File | Action | Purpose |
|---|---|---|
| `ui/src/components/CommandPalette.svelte` | **Create** | Fuzzy-search overlay of all actions |
| `ui/src/components/AppShell.svelte` | **Modify** | Add global `keydown` handler + render CommandPalette |
| `ui/src/components/DiffViewer.svelte` | **Modify** | Add split-view toggle + render BlameViewer |
| `ui/src/components/BlameViewer.svelte` | **Create** | Per-line blame gutter with author/hash/date |
| `src/remotes/mod.rs` | **Modify** | Add `clone_repo` async function |
| `src-tauri/src/lib.rs` | **Modify** | Expose `clone_repo` as Tauri command |
| `ui/src/lib/tauri.ts` | **Modify** | Add `cloneRepo` frontend wrapper |
| `ui/src/components/Welcome.svelte` | **Modify** | Wire clone button to real command |

---

## Task 1: CommandPalette Component

**Files:**
- Create: `ui/src/components/CommandPalette.svelte`

- [ ] **Step 1: Create CommandPalette.svelte**

```svelte
<script lang="ts">
	import { currentRepo, fetchFromRemote } from '$lib/store';
	import { createCommit, push } from '$lib/tauri';
	import { get } from 'svelte/store';

	let { onclose }: { onclose: () => void } = $props();

	interface Action {
		id: string;
		label: string;
		shortcut?: string;
		run: () => Promise<void> | void;
	}

	const ACTIONS: Action[] = [
		{
			id: 'push',
			label: 'Push',
			shortcut: '⌘P',
			run: async () => {
				const repo = get(currentRepo);
				if (!repo) return;
				await push(repo, 'origin', '');
			}
		},
		{
			id: 'fetch',
			label: 'Fetch',
			shortcut: '⌘F',
			run: async () => {
				await fetchFromRemote('origin');
			}
		},
	];

	let query = $state('');
	let activeIdx = $state(0);

	const filtered = $derived(
		query.trim() === ''
			? ACTIONS
			: ACTIONS.filter((a) => a.label.toLowerCase().includes(query.toLowerCase()))
	);

	$effect(() => {
		activeIdx = 0;
	});

	function handleKey(e: KeyboardEvent) {
		if (e.key === 'Escape') { e.preventDefault(); onclose(); return; }
		if (e.key === 'ArrowDown') { e.preventDefault(); activeIdx = Math.min(activeIdx + 1, filtered.length - 1); return; }
		if (e.key === 'ArrowUp') { e.preventDefault(); activeIdx = Math.max(activeIdx - 1, 0); return; }
		if (e.key === 'Enter') { e.preventDefault(); runAction(filtered[activeIdx]); return; }
	}

	async function runAction(action: Action | undefined) {
		if (!action) return;
		onclose();
		await action.run();
	}
</script>

<div class="palette-backdrop" onclick={onclose} role="none">
	<div class="palette" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={handleKey}>
		<input
			class="palette-input"
			type="text"
			placeholder="Type a command..."
			bind:value={query}
			autofocus
		/>
		<ul class="palette-list" role="listbox">
			{#each filtered as action, i (action.id)}
				<li
					class="palette-item"
					class:active={i === activeIdx}
					role="option"
					aria-selected={i === activeIdx}
					onclick={() => runAction(action)}
					onmouseenter={() => (activeIdx = i)}
				>
					<span class="palette-label">{action.label}</span>
					{#if action.shortcut}
						<span class="palette-shortcut">{action.shortcut}</span>
					{/if}
				</li>
			{/each}
			{#if filtered.length === 0}
				<li class="palette-empty">No commands found</li>
			{/if}
		</ul>
	</div>
</div>

<style>
	.palette-backdrop {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.5);
		z-index: 2000;
		display: flex;
		align-items: flex-start;
		justify-content: center;
		padding-top: 80px;
	}
	.palette {
		background: var(--bg-secondary);
		border: 1px solid var(--border);
		border-radius: 8px;
		width: 480px;
		max-height: 360px;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		box-shadow: 0 16px 48px rgba(0, 0, 0, 0.5);
	}
	.palette-input {
		background: transparent;
		border: none;
		border-bottom: 1px solid var(--border);
		padding: 12px 16px;
		color: var(--text-primary);
		font-size: 14px;
		outline: none;
	}
	.palette-list {
		list-style: none;
		margin: 0;
		padding: 4px;
		overflow-y: auto;
	}
	.palette-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 8px 12px;
		border-radius: 4px;
		cursor: pointer;
		font-size: 13px;
		color: var(--text-primary);
	}
	.palette-item.active {
		background: var(--accent-blue);
		color: white;
	}
	.palette-shortcut {
		font-size: 11px;
		opacity: 0.6;
		font-family: monospace;
	}
	.palette-empty {
		padding: 16px;
		text-align: center;
		color: var(--text-muted);
		font-size: 13px;
	}
</style>
```

- [ ] **Step 2: Verify TypeScript compiles**

```bash
cd /Users/sarvadhisolution/Documents/Personal/GitAxon/ui && npx tsc --noEmit
```
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add ui/src/components/CommandPalette.svelte
git commit -m "feat(ux): add CommandPalette component with fuzzy action search"
```

---

## Task 2: Global Keyboard Shortcuts in AppShell

**Files:**
- Modify: `ui/src/components/AppShell.svelte`

- [ ] **Step 1: Add palette state and keydown handler**

In `AppShell.svelte`, add to the `<script>` block (after existing imports):

```svelte
import CommandPalette from './CommandPalette.svelte';
import { fetchFromRemote } from '$lib/store';
import { get } from 'svelte/store';

let paletteOpen = $state(false);

function handleGlobalKey(e: KeyboardEvent) {
    if (!e.metaKey && !e.ctrlKey) return;
    const key = e.key.toLowerCase();
    // ⌘K — command palette
    if (key === 'k') { e.preventDefault(); paletteOpen = true; return; }
    // ⌘F — fetch
    if (key === 'f') { e.preventDefault(); fetchFromRemote('origin'); return; }
    // ⌘P — push  (skip if input focused)
    if (key === 'p' && !(document.activeElement instanceof HTMLInputElement || document.activeElement instanceof HTMLTextAreaElement)) {
        e.preventDefault();
        const repo = get(currentRepo);
        if (repo) import('$lib/tauri').then(({ push }) => push(repo, 'origin', ''));
        return;
    }
}
```

- [ ] **Step 2: Wire handler and render palette in template**

In the template of `AppShell.svelte`, add before the closing `</div>`:

```svelte
<svelte:window onkeydown={handleGlobalKey} />
{#if paletteOpen}
    <CommandPalette onclose={() => (paletteOpen = false)} />
{/if}
```

- [ ] **Step 3: Verify TypeScript compiles**

```bash
cd /Users/sarvadhisolution/Documents/Personal/GitAxon/ui && npx tsc --noEmit
```
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add ui/src/components/AppShell.svelte
git commit -m "feat(ux): global keyboard shortcuts — ⌘K palette, ⌘F fetch, ⌘P push"
```

---

## Task 3: Side-by-Side Diff View

**Files:**
- Modify: `ui/src/components/DiffViewer.svelte`

- [ ] **Step 1: Add splitMode state and toggle button**

In `DiffViewer.svelte` `<script>`, add:

```typescript
let splitMode = $state(false);
```

In the toolbar (near the existing `File View` / `Diff View` buttons), add:

```svelte
<button
    class="toolbar-btn"
    class:active={splitMode}
    onclick={() => (splitMode = !splitMode)}
    title="Toggle split view"
>⇔ Split</button>
```

- [ ] **Step 2: Implement split layout**

Find the section in `DiffViewer.svelte` that renders hunks in unified mode. Wrap it with a conditional:

```svelte
{#if splitMode}
    <div class="split-view">
        <div class="split-pane split-left" bind:this={leftPane} onscroll={syncScroll}>
            {#each file.hunks as hunk}
                <div class="hunk-header">@@ -{hunk.old_start},{hunk.old_lines} ...</div>
                {#each hunk.lines as line}
                    {#if line.line_type === 'Deleted' || line.line_type === 'Context'}
                        <div class="diff-line {line.line_type === 'Deleted' ? 'line-deleted' : 'line-context'}">
                            <span class="ln">{line.old_line_no ?? ''}</span>
                            <span class="content">{line.content}</span>
                        </div>
                    {/if}
                {/each}
            {/each}
        </div>
        <div class="split-pane split-right" bind:this={rightPane} onscroll={syncScroll}>
            {#each file.hunks as hunk}
                <div class="hunk-header">@@ +{hunk.new_start},{hunk.new_lines} ...</div>
                {#each hunk.lines as line}
                    {#if line.line_type === 'Added' || line.line_type === 'Context'}
                        <div class="diff-line {line.line_type === 'Added' ? 'line-added' : 'line-context'}">
                            <span class="ln">{line.new_line_no ?? ''}</span>
                            <span class="content">{line.content}</span>
                        </div>
                    {/if}
                {/each}
            {/each}
        </div>
    </div>
{:else}
    <!-- existing unified diff render here (unchanged) -->
{/if}
```

- [ ] **Step 3: Add scroll sync and split CSS**

In `<script>`:
```typescript
let leftPane = $state<HTMLDivElement | null>(null);
let rightPane = $state<HTMLDivElement | null>(null);
let syncingScroll = false;

function syncScroll(e: Event) {
    if (syncingScroll) return;
    syncingScroll = true;
    const src = e.target as HTMLDivElement;
    const other = src === leftPane ? rightPane : leftPane;
    if (other) other.scrollTop = src.scrollTop;
    syncingScroll = false;
}
```

In `<style>`:
```css
.split-view {
    display: flex;
    height: 100%;
    overflow: hidden;
}
.split-pane {
    flex: 1;
    overflow: auto;
    font-family: monospace;
    font-size: 12px;
}
.split-left {
    border-right: 1px solid var(--border);
}
```

- [ ] **Step 4: Verify TypeScript compiles**

```bash
cd /Users/sarvadhisolution/Documents/Personal/GitAxon/ui && npx tsc --noEmit
```
Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/DiffViewer.svelte
git commit -m "feat(ux): side-by-side split diff view with scroll sync"
```

---

## Task 4: BlameViewer Component

**Files:**
- Create: `ui/src/components/BlameViewer.svelte`
- Modify: `ui/src/components/DiffViewer.svelte`

The Rust backend (`git_blame`), tauri command, and `gitBlame` in tauri.ts already exist. `DiffViewer.svelte` already has `blameMode`, `blameData`, `blameLoading`, and `toggleBlame()`. This task creates the rendering component and wires it into the DiffViewer tab bar.

- [ ] **Step 1: Create BlameViewer.svelte**

```svelte
<script lang="ts">
	import type { BlameLine } from '$lib/tauri';

	let {
		lines,
		onhashclick
	}: {
		lines: BlameLine[];
		onhashclick: (hash: string) => void;
	} = $props();

	function relativeDate(timestamp: number): string {
		const diff = Math.floor((Date.now() / 1000) - timestamp);
		if (diff < 60) return 'just now';
		if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
		if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
		if (diff < 2592000) return `${Math.floor(diff / 86400)}d ago`;
		return new Date(timestamp * 1000).toLocaleDateString();
	}
</script>

<div class="blame-viewer">
	{#each lines as line (line.line_no)}
		<div class="blame-row">
			<div class="blame-gutter">
				<button
					class="blame-hash"
					onclick={() => onhashclick(line.commit_hash)}
					title="{line.summary}\n{line.author} · {line.date}"
				>{line.short_hash}</button>
				<span class="blame-author">{line.author.split(' ')[0]}</span>
				<span class="blame-date">{relativeDate(line.timestamp)}</span>
			</div>
			<span class="blame-line-no">{line.line_no}</span>
			<span class="blame-content">{line.content}</span>
		</div>
	{/each}
</div>

<style>
	.blame-viewer {
		font-family: monospace;
		font-size: 12px;
		overflow: auto;
		height: 100%;
		background: var(--bg-primary);
	}
	.blame-row {
		display: flex;
		align-items: baseline;
		min-height: 18px;
		border-bottom: 1px solid transparent;
	}
	.blame-row:hover {
		background: var(--bg-secondary);
	}
	.blame-gutter {
		display: flex;
		gap: 6px;
		padding: 1px 8px;
		min-width: 220px;
		max-width: 220px;
		background: var(--bg-secondary);
		border-right: 1px solid var(--border);
		overflow: hidden;
	}
	.blame-hash {
		background: none;
		border: none;
		color: var(--accent-blue);
		font-family: monospace;
		font-size: 11px;
		cursor: pointer;
		padding: 0;
		flex-shrink: 0;
	}
	.blame-hash:hover { text-decoration: underline; }
	.blame-author {
		color: var(--text-secondary);
		font-size: 11px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		flex: 1;
	}
	.blame-date {
		color: var(--text-muted);
		font-size: 10px;
		flex-shrink: 0;
	}
	.blame-line-no {
		color: var(--text-muted);
		padding: 0 8px;
		min-width: 40px;
		text-align: right;
		user-select: none;
	}
	.blame-content {
		color: var(--text-primary);
		white-space: pre;
		flex: 1;
	}
</style>
```

- [ ] **Step 2: Wire BlameViewer into DiffViewer**

In `DiffViewer.svelte`, import and render `BlameViewer`. Find where `blameMode` is already handled and add:

```svelte
import BlameViewer from './BlameViewer.svelte';
```

And in the template where `blameMode` is checked (look for `{#if blameMode}`), ensure it renders:

```svelte
{#if blameMode}
    {#if blameLoading}
        <div class="blame-loading">Loading blame...</div>
    {:else}
        <BlameViewer
            lines={blameData}
            onhashclick={(hash) => {
                const commit = commitList.find(c => c.hash.startsWith(hash));
                if (commit) selectCommit(commit);
            }}
        />
    {/if}
{/if}
```

Import `selectCommit` and `commits` from store if not already imported:
```typescript
import { selectCommit, commits } from '$lib/store';
import { get } from 'svelte/store';
const commitList = get(commits);
```

- [ ] **Step 3: Verify TypeScript compiles**

```bash
cd /Users/sarvadhisolution/Documents/Personal/GitAxon/ui && npx tsc --noEmit
```
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add ui/src/components/BlameViewer.svelte ui/src/components/DiffViewer.svelte
git commit -m "feat(ux): git blame viewer with per-line author/hash/date gutter"
```

---

## Task 5: Clone Repo — Rust Backend

**Files:**
- Modify: `src/remotes/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add `clone_repo` to `src/remotes/mod.rs`**

Add after existing imports (ensure `git2::build::RepoBuilder` is available — `git2` is already a dep):

```rust
/// Clone a remote repository to a local destination path.
pub async fn clone_repo(url: String, dest: String) -> Result<(), GitfastError> {
    tokio::task::spawn_blocking(move || {
        git2::build::RepoBuilder::new()
            .clone(&url, std::path::Path::new(&dest))
            .map(|_| ())
            .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))
    })
    .await
    .map_err(|e| GitfastError::GitOperationFailed(e.to_string()))?
}
```

- [ ] **Step 2: Expose as Tauri command in `src-tauri/src/lib.rs`**

Add the command handler (near other remote commands):

```rust
#[tauri::command]
async fn clone_repo(url: String, dest: String) -> Result<(), String> {
    gitaxon::remotes::clone_repo(url, dest)
        .await
        .map_err(|e| e.to_string())
}
```

Register in the `invoke_handler` builder:
```rust
clone_repo,   // add to the existing list
```

- [ ] **Step 3: Build Rust to verify**

```bash
cargo build 2>&1 | grep "^error"
```
Expected: no output (no errors)

- [ ] **Step 4: Commit**

```bash
git add src/remotes/mod.rs src-tauri/src/lib.rs
git commit -m "feat(ux): add clone_repo Rust command via git2 RepoBuilder"
```

---

## Task 6: Wire Clone Button in Welcome Screen

**Files:**
- Modify: `ui/src/lib/tauri.ts`
- Modify: `ui/src/components/Welcome.svelte`

- [ ] **Step 1: Add `cloneRepo` to `ui/src/lib/tauri.ts`**

```typescript
export async function cloneRepo(url: string, dest: string): Promise<void> {
    await invoke('clone_repo', { url, dest });
}
```

- [ ] **Step 2: Wire clone button in `Welcome.svelte`**

Add import at top of script:
```typescript
import { cloneRepo } from '$lib/tauri';
import { open } from '@tauri-apps/plugin-dialog';
import { loadRepo } from '$lib/store';
import { showToast } from '$lib/toast';
```

Replace `showComingSoon` references for the clone button with a real handler:

```typescript
let cloneLoading = $state(false);

async function handleClone() {
    if (!cloneUrl.trim()) return;
    const dest = await open({ directory: true, multiple: false });
    if (!dest || typeof dest !== 'string') return;
    cloneLoading = true;
    try {
        // destination = chosen folder + repo name from URL
        const repoName = cloneUrl.split('/').pop()?.replace(/\.git$/, '') ?? 'repo';
        const fullDest = `${dest}/${repoName}`;
        await cloneRepo(cloneUrl, fullDest);
        showToast(`Cloned to ${fullDest}`, 'success');
        await loadRepo(fullDest);
        cloneUrl = '';
    } catch (e) {
        showToast(String(e), 'error');
    } finally {
        cloneLoading = false;
    }
}
```

Update the clone button's `onclick` to `handleClone` and show loading state:
```svelte
<button class="btn-clone" onclick={handleClone} disabled={cloneLoading}>
    {cloneLoading ? 'Cloning...' : 'Clone'}
</button>
```

- [ ] **Step 3: Verify TypeScript compiles**

```bash
cd /Users/sarvadhisolution/Documents/Personal/GitAxon/ui && npx tsc --noEmit
```
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add ui/src/lib/tauri.ts ui/src/components/Welcome.svelte
git commit -m "feat(ux): wire clone button in Welcome screen — picks dest folder, clones, opens repo"
```
