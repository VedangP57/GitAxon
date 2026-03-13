<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import CommitGraph from './CommitGraph.svelte';
	import StagingPanel from './StagingPanel.svelte';
	import DiffPanel from './DiffPanel.svelte';
	import BranchSidebar from './BranchSidebar.svelte';
	import Toast from './Toast.svelte';
	import {
		loadRepo,
		currentRepo,
		isLoading,
		fetchFromRemote,
		branches
	} from '$lib/store';
	import { pull, push } from '$lib/tauri';
	import { showToast } from '$lib/toast';

	let repoName = $derived(
		$currentRepo ? $currentRepo.split(/[/\\]/).filter(Boolean).pop() ?? 'Repository' : 'Repository'
	);

	const currentBranchName = $derived(
		$branches.find((b) => b.isHead && !b.isRemote)?.name ?? ''
	);

	const SIDEBAR_MIN = 180;
	const SIDEBAR_MAX = 400;
	const DIFF_MIN = 200;
	const DIFF_MAX = 600;

	function getStoredWidth(key: string, defaultVal: number): number {
		if (typeof window === 'undefined') return defaultVal;
		try {
			const v = localStorage.getItem(key);
			if (v) {
				const n = parseInt(v, 10);
				if (!isNaN(n)) return n;
			}
		} catch {}
		return defaultVal;
	}

	function setStoredWidth(key: string, val: number) {
		try {
			localStorage.setItem(key, String(val));
		} catch {}
	}

	let sidebarWidth = $state(getStoredWidth('gitfast-sidebar-width', 260));
	let diffWidth = $state(getStoredWidth('gitfast-diff-width', 320));
	let resizing = $state<'left' | 'right' | null>(null);

	function onResizeStart(which: 'left' | 'right') {
		resizing = which;
	}

	function onMouseMove(e: MouseEvent) {
		if (resizing === 'left') {
			const w = Math.max(SIDEBAR_MIN, Math.min(SIDEBAR_MAX, e.clientX));
			sidebarWidth = w;
			setStoredWidth('gitfast-sidebar-width', w);
		} else if (resizing === 'right') {
			// Diff panel is on the right; its width = distance from handle to right edge
			const w = Math.max(
				DIFF_MIN,
				Math.min(DIFF_MAX, window.innerWidth - e.clientX)
			);
			diffWidth = w;
			setStoredWidth('gitfast-diff-width', w);
		}
	}

	function onMouseUp() {
		resizing = null;
	}

	onMount(() => {
		window.addEventListener('mousemove', onMouseMove);
		window.addEventListener('mouseup', onMouseUp);
	});

	onDestroy(() => {
		window.removeEventListener('mousemove', onMouseMove);
		window.removeEventListener('mouseup', onMouseUp);
	});

	$effect(() => {
		const name = repoName;
		getCurrentWindow().setTitle('GitFast — ' + name);
	});

	onMount(() => {
		if ($currentRepo) loadRepo($currentRepo);
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
</script>

<div class="app-shell">
	<!-- Top toolbar -->
	<header class="toolbar">
		<div class="toolbar-left">{repoName}</div>
		<div class="toolbar-center">
			<button class="toolbar-btn" onclick={onFetch}>Fetch</button>
			<button class="toolbar-btn" onclick={onPull}>Pull</button>
			<button class="toolbar-btn" onclick={onPush}>Push</button>
		</div>
		<div class="toolbar-right">
			{#if $isLoading}
				<div class="spinner" aria-label="Loading"></div>
			{/if}
		</div>
	</header>

	<!-- Main content -->
	<div class="main-content">
		<aside class="sidebar-left" style="width: {sidebarWidth}px;">
			<BranchSidebar />
		</aside>
		<div
			class="resize-handle"
			role="separator"
			aria-orientation="vertical"
			onmousedown={() => onResizeStart('left')}
		></div>

		<main class="center-panel">
			<div class="center-top">
				<CommitGraph />
			</div>
			<div class="center-bottom">
				<StagingPanel />
			</div>
		</main>

		<div
			class="resize-handle"
			role="separator"
			aria-orientation="vertical"
			onmousedown={() => onResizeStart('right')}
		></div>
		<aside class="sidebar-right" style="width: {diffWidth}px;">
			<DiffPanel />
		</aside>
	</div>

	<Toast />
</div>

<style>
	.app-shell {
		display: flex;
		flex-direction: column;
		height: 100vh;
		background: var(--bg-primary);
	}

	.toolbar {
		height: 48px;
		min-height: 48px;
		background: var(--bg-secondary);
		border-bottom: 1px solid var(--border);
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0 16px;
	}

	.toolbar-left {
		font-weight: 600;
		color: var(--text-primary);
	}

	.toolbar-center {
		display: flex;
		gap: 8px;
	}

	.toolbar-btn {
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		color: var(--text-primary);
		padding: 6px 14px;
		border-radius: 6px;
		font-size: 13px;
		cursor: pointer;
	}
	.toolbar-btn:hover {
		background: var(--border);
	}

	.toolbar-right {
		min-width: 24px;
	}

	.spinner {
		width: 20px;
		height: 20px;
		border: 2px solid var(--border);
		border-top-color: var(--accent-blue);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}
	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.main-content {
		flex: 1;
		display: flex;
		flex-direction: row;
		min-height: 0;
	}

	.sidebar-left {
		min-width: 180px;
		max-width: 400px;
		flex-shrink: 0;
		background: var(--bg-secondary);
		border-right: 1px solid var(--border);
		overflow: auto;
		padding: 12px;
	}

	.resize-handle {
		width: 4px;
		flex-shrink: 0;
		background: var(--border);
		cursor: col-resize;
	}
	.resize-handle:hover {
		background: var(--accent-blue);
	}

	.center-panel {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-width: 0;
		min-height: 0;
		overflow: hidden;
	}

	.center-top {
		flex: 0 0 60%;
		min-height: 0;
		overflow: auto;
		padding: 12px;
	}

	.center-bottom {
		flex: 0 0 40%;
		min-height: 0;
		overflow: auto;
		padding: 12px;
		border-top: 1px solid var(--border);
	}

	.sidebar-right {
		min-width: 200px;
		max-width: 600px;
		flex-shrink: 0;
		background: var(--bg-secondary);
		border-left: 1px solid var(--border);
		overflow: auto;
		padding: 12px;
	}

	:global(.placeholder) {
		width: 100%;
		height: 100%;
		min-height: 120px;
	}
</style>
