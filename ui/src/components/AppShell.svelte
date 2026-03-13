<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import CenterPanel from './CenterPanel.svelte';
	import RightPanel from './RightPanel.svelte';
	import BranchSidebar from './BranchSidebar.svelte';
	import Toast from './Toast.svelte';
	import {
		loadRepo,
		currentRepo,
		isLoading,
		fetchFromRemote,
		branches,
		goHome
	} from '$lib/store';
	import { pull, push, stashPush, stashPop, openTerminalAt } from '$lib/tauri';
	import { showToast } from '$lib/toast';
	import { openCreateBranchForm } from '$lib/store';

	let repoName = $derived(
		$currentRepo ? $currentRepo.split(/[/\\]/).filter(Boolean).pop() ?? 'Repository' : 'Repository'
	);

	const currentBranchName = $derived(
		$branches.find((b) => b.isHead && !b.isRemote)?.name ?? ''
	);

	// ── Resizable panels ─────────────────────────────────────────
	const LEFT_MIN = 160;
	const LEFT_MAX = 400;
	const RIGHT_MIN = 220;
	const RIGHT_MAX = 500;

	function stored(key: string, def: number): number {
		try { const v = localStorage.getItem(key); if (v) { const n = parseInt(v, 10); if (!isNaN(n)) return n; } } catch {}
		return def;
	}
	function store(key: string, val: number) {
		try { localStorage.setItem(key, String(val)); } catch {}
	}

	let leftWidth  = $state(stored('gax-left',  180));
	let rightWidth = $state(stored('gax-right', 300));
	let resizing   = $state<'left' | 'right' | null>(null);

	function startResize(which: 'left' | 'right') {
		resizing = which;
	}

	function onMouseMove(e: MouseEvent) {
		if (resizing === 'left') {
			const w = Math.max(LEFT_MIN, Math.min(LEFT_MAX, e.clientX));
			leftWidth = w;
			store('gax-left', w);
		} else if (resizing === 'right') {
			const w = Math.max(RIGHT_MIN, Math.min(RIGHT_MAX, window.innerWidth - e.clientX));
			rightWidth = w;
			store('gax-right', w);
		}
	}

	function onMouseUp() {
		resizing = null;
	}

	onMount(() => {
		window.addEventListener('mousemove', onMouseMove);
		window.addEventListener('mouseup', onMouseUp);
		if ($currentRepo) loadRepo($currentRepo);
	});

	onDestroy(() => {
		window.removeEventListener('mousemove', onMouseMove);
		window.removeEventListener('mouseup', onMouseUp);
	});
	// ─────────────────────────────────────────────────────────────

	$effect(() => {
		const name = repoName;
		getCurrentWindow().setTitle('GitAxon — ' + name);
	});

	async function onFetch() {
		const repo = $currentRepo;
		if (!repo) return;
		try {
			await fetchFromRemote('origin');
			showToast('Fetch completed', 'success');
		} catch (e) {
			showToast(e instanceof Error ? e.message : String(e), 'error');
		}
	}

	async function onPull() {
		const repo = $currentRepo;
		if (!repo || !currentBranchName) {
			showToast('No branch selected or detached HEAD', 'error');
			return;
		}
		try {
			await pull(repo, 'origin', currentBranchName);
			await loadRepo(repo);
			showToast('Pull completed', 'success');
		} catch (e) {
			showToast(e instanceof Error ? e.message : String(e), 'error');
		}
	}

	async function onPush() {
		const repo = $currentRepo;
		if (!repo || !currentBranchName) {
			showToast('No branch selected or detached HEAD', 'error');
			return;
		}
		try {
			await push(repo, 'origin', currentBranchName, false);
			await loadRepo(repo);
			showToast('Push completed', 'success');
		} catch (e) {
			showToast(e instanceof Error ? e.message : String(e), 'error');
		}
	}

	async function onStash() {
		const repo = $currentRepo;
		if (!repo) return;
		try {
			await stashPush(repo);
			await loadRepo(repo);
			showToast('Changes stashed', 'success');
		} catch (e) {
			showToast(e instanceof Error ? e.message : String(e), 'error');
		}
	}

	async function onPop() {
		const repo = $currentRepo;
		if (!repo) return;
		try {
			await stashPop(repo);
			await loadRepo(repo);
			showToast('Stash applied', 'success');
		} catch (e) {
			showToast(e instanceof Error ? e.message : String(e), 'error');
		}
	}

	function onTerminal() {
		const repo = $currentRepo;
		if (!repo) {
			showToast('No repository open', 'error');
			return;
		}
		openTerminalAt(repo).catch((e) =>
			showToast(e instanceof Error ? e.message : String(e), 'error')
		);
	}

	function onUndo() {
		showToast('Undo not implemented', 'info');
	}

	function onRedo() {
		showToast('Redo not implemented', 'info');
	}

	let branchDropdownOpen = $state(false);
	function toggleBranchDropdown() {
		branchDropdownOpen = !branchDropdownOpen;
	}
	function openCreateBranch() {
		openCreateBranchForm.set(true);
		branchDropdownOpen = false;
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="app-shell"
	class:is-resizing={resizing !== null}
>
	<!-- ═══ TOOLBAR ═══ -->
	<header class="toolbar">
		<!-- Left -->
		<div class="tb-left">
			<button class="tb-icon-btn" onclick={goHome} title="Back to Home">
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
					<path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/>
				</svg>
			</button>
			<span class="tb-repo-name">{repoName}</span>
			{#if currentBranchName}
				<span class="tb-sep">›</span>
				<span class="tb-branch-name">{currentBranchName}</span>
			{/if}
			<button class="tb-icon-btn" onclick={onFetch} title="Fetch">
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
					<path d="M23 4v6h-6"/><path d="M1 20v-6h6"/><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
				</svg>
			</button>
		</div>

		<!-- Center action buttons -->
		<div class="tb-center">
			<button class="tb-action-btn" title="Undo" onclick={onUndo}>
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7v6h6"/><path d="M21 17a9 9 0 0 0-9-9 9 9 0 0 0-6 2.3L3 13"/></svg>
				<span>Undo</span>
			</button>
			<button class="tb-action-btn" title="Redo" onclick={onRedo}>
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 7v6h-6"/><path d="M3 17a9 9 0 0 1 9-9 9 9 0 0 1 6 2.3L21 13"/></svg>
				<span>Redo</span>
			</button>
			<div class="tb-divider"></div>
			<button class="tb-action-btn" onclick={onPull} title="Pull">
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 5v14"/><polyline points="19 12 12 19 5 12"/></svg>
				<span>Pull</span>
			</button>
			<button class="tb-action-btn" onclick={onPush} title="Push">
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 19V5"/><polyline points="5 12 12 5 19 12"/></svg>
				<span>Push</span>
			</button>
			<div class="tb-divider"></div>
			<div class="tb-dropdown-wrap">
				<button class="tb-action-btn" title="Branch" onclick={toggleBranchDropdown} aria-expanded={branchDropdownOpen} aria-haspopup="true">
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="6" y1="3" x2="6" y2="15"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M18 9a9 9 0 0 1-9 9"/></svg>
					<span>Branch ▾</span>
				</button>
				{#if branchDropdownOpen}
					<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
					<div class="tb-dropdown-backdrop" onclick={() => (branchDropdownOpen = false)} role="presentation"></div>
					<div class="tb-dropdown" role="menu">
						<button class="tb-dropdown-item" onclick={openCreateBranch} role="menuitem">Create branch...</button>
						<span class="tb-dropdown-hint">Switch/delete in left sidebar</span>
					</div>
				{/if}
			</div>
			<button class="tb-action-btn" title="Stash" onclick={onStash}>
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="7" width="20" height="14" rx="2" ry="2"/><path d="M16 21V5a2 2 0 0 0-2-2h-4a2 2 0 0 0-2 2v16"/></svg>
				<span>Stash</span>
			</button>
			<button class="tb-action-btn" title="Pop stash" onclick={onPop}>
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 5v14"/><polyline points="19 12 12 5 5 12"/></svg>
				<span>Pop</span>
			</button>
			<div class="tb-divider"></div>
			<button class="tb-action-btn" title="Terminal" onclick={onTerminal}>
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>
				<span>Terminal</span>
			</button>
		</div>

		<!-- Right icons + spinner -->
		<div class="tb-right">
			{#if $isLoading}
				<div class="spinner" aria-label="Loading"></div>
			{/if}
			<button class="tb-icon-btn" title="Notifications">
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 0 1-3.46 0"/></svg>
			</button>
			<button class="tb-icon-btn" title="Settings">
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
			</button>
			<button class="tb-profile-btn" title="Profile">
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>
			</button>
		</div>
	</header>

	<!-- ═══ MAIN BODY (flex row) ═══ -->
	<div class="main-body">
		<!-- ZONE 1: LEFT SIDEBAR -->
		<aside class="left-panel" style="width: {leftWidth}px;">
			<BranchSidebar />
		</aside>

		<!-- Left resize handle -->
		<div
			class="resize-handle"
			class:active={resizing === 'left'}
			role="separator"
			aria-orientation="vertical"
			onmousedown={() => startResize('left')}
		>
			<span class="resize-dots">⋮</span>
		</div>

		<!-- ZONE 2: CENTER -->
		<main class="center">
			<CenterPanel />
		</main>

		<!-- Right resize handle -->
		<div
			class="resize-handle"
			class:active={resizing === 'right'}
			role="separator"
			aria-orientation="vertical"
			onmousedown={() => startResize('right')}
		>
			<span class="resize-dots">⋮</span>
		</div>

		<!-- ZONE 3: RIGHT PANEL -->
		<aside class="right-panel" style="width: {rightWidth}px;">
			<RightPanel />
		</aside>
	</div>

	<Toast />
</div>

<style>
	.app-shell {
		display: flex;
		flex-direction: column;
		height: 100vh;
		overflow: hidden;
		background: #0d1117;
	}

	/* Disable text selection while dragging */
	.app-shell.is-resizing {
		user-select: none;
		cursor: col-resize;
	}
	.app-shell.is-resizing * {
		pointer-events: none;
	}
	.app-shell.is-resizing .resize-handle {
		pointer-events: all;
	}

	/* ── Toolbar ── */
	.toolbar {
		height: 48px;
		min-height: 48px;
		background: var(--bg-secondary);
		border-bottom: 1px solid var(--border);
		display: flex;
		align-items: center;
		padding: 0 8px;
		flex-shrink: 0;
		-webkit-app-region: drag;
	}

	.tb-left {
		display: flex;
		align-items: center;
		gap: 6px;
		flex-shrink: 0;
		-webkit-app-region: no-drag;
	}

	.tb-center {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 2px;
		-webkit-app-region: no-drag;
	}

	.tb-right {
		display: flex;
		align-items: center;
		gap: 4px;
		flex-shrink: 0;
		-webkit-app-region: no-drag;
	}

	.tb-repo-name {
		font-size: 13px;
		font-weight: 600;
		color: var(--text-primary);
		white-space: nowrap;
	}

	.tb-sep { color: var(--text-muted); font-size: 14px; }

	.tb-branch-name {
		font-size: 12px;
		color: var(--text-muted);
		white-space: nowrap;
		max-width: 120px;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.tb-icon-btn {
		width: 30px;
		height: 30px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: transparent;
		border: 1px solid transparent;
		border-radius: 4px;
		color: var(--text-muted);
		cursor: pointer;
		transition: color 0.1s, background 0.1s;
		flex-shrink: 0;
	}
	.tb-icon-btn:hover {
		color: var(--text-primary);
		background: var(--bg-tertiary);
		border-color: var(--border);
	}

	.tb-action-btn {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 2px;
		padding: 0 8px;
		height: 48px;
		min-width: 44px;
		background: transparent;
		border: none;
		border-radius: 0;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 11px;
		transition: color 0.1s, background 0.1s;
	}
	.tb-action-btn span { font-size: 10px; white-space: nowrap; }
	.tb-action-btn:hover { color: var(--text-primary); background: var(--bg-tertiary); }

	.tb-divider {
		width: 1px;
		height: 28px;
		background: var(--border);
		margin: 0 4px;
		flex-shrink: 0;
	}

	.tb-dropdown-wrap {
		position: relative;
	}
	.tb-dropdown-backdrop {
		position: fixed;
		inset: 0;
		z-index: 99;
	}
	.tb-dropdown {
		position: absolute;
		top: 100%;
		left: 0;
		margin-top: 2px;
		min-width: 200px;
		padding: 4px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 6px;
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
		z-index: 100;
	}
	.tb-dropdown-item {
		display: block;
		width: 100%;
		padding: 8px 12px;
		background: none;
		border: none;
		color: var(--text-primary);
		font-size: 12px;
		text-align: left;
		cursor: pointer;
		border-radius: 4px;
	}
	.tb-dropdown-item:hover {
		background: var(--bg-secondary);
	}
	.tb-dropdown-hint {
		display: block;
		padding: 6px 12px;
		font-size: 10px;
		color: var(--text-muted);
	}

	.tb-profile-btn {
		width: 28px;
		height: 28px;
		border-radius: 50%;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--text-secondary);
		cursor: pointer;
	}
	.tb-profile-btn:hover { border-color: var(--text-muted); }

	/* ── Main body: flex row ── */
	.main-body {
		flex: 1;
		display: flex;
		flex-direction: row;
		min-height: 0;
		overflow: hidden;
	}

	/* ── Panels ── */
	.left-panel {
		flex-shrink: 0;
		overflow: hidden;
		min-width: 160px;
		max-width: 400px;
	}

	.center {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		position: relative;
	}

	.right-panel {
		flex-shrink: 0;
		overflow: hidden;
		min-width: 220px;
		max-width: 500px;
	}

	/* ── Resize handles ── */
	.resize-handle {
		flex-shrink: 0;
		width: 1px;
		background: var(--border);
		cursor: col-resize;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: background 0.15s, width 0.15s;
		position: relative;
		z-index: 10;
	}

	.resize-handle:hover,
	.resize-handle.active {
		background: var(--text-muted);
		width: 3px;
	}

	.resize-dots {
		color: var(--text-muted);
		font-size: 11px;
		line-height: 1;
		opacity: 0;
		transition: opacity 0.15s;
		pointer-events: none;
		writing-mode: vertical-lr;
		letter-spacing: -3px;
	}
	.resize-handle:hover .resize-dots,
	.resize-handle.active .resize-dots {
		opacity: 1;
	}

	/* Spinner */
	.spinner {
		width: 16px;
		height: 16px;
		border: 2px solid var(--border);
		border-top-color: var(--text-secondary);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
		flex-shrink: 0;
	}
	@keyframes spin { to { transform: rotate(360deg); } }
</style>
