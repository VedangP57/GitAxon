<script lang="ts">
	import { get } from 'svelte/store';
	import { onMount, onDestroy } from 'svelte';
	import { currentRepo, closeDiff } from '$lib/store';
	import {
		getRebaseState,
		getRebaseTodoForRange,
		startInteractiveRebase,
		continueRebase,
		abortRebase,
		skipRebase
	} from '$lib/tauri';
	import { showToast } from '$lib/toast';
	import type { RebaseTodoItem, RebaseState } from '$lib/types';

	const ACTIONS = ['pick', 'squash', 'fixup', 'reword', 'edit', 'drop'] as const;

	let rebaseState = $state<RebaseState | null>(null);
	let todoItems = $state<RebaseTodoItem[]>([]);
	let ontoHash = $state('');
	let isWorking = $state(false);
	let mode = $state<'setup' | 'progress'>('setup');
	let dragIndex = $state<number | null>(null);

	/** Called from CommitGraph context menu — sets up the todo list for review. */
	export async function setupRebase(targetHash: string) {
		const repo = get(currentRepo);
		if (!repo) return;
		ontoHash = targetHash;
		mode = 'setup';
		isWorking = true;
		try {
			todoItems = await getRebaseTodoForRange(repo, targetHash);
			if (todoItems.length === 0) {
				showToast('No commits to rebase onto this target', 'error');
				closeDiff();
			}
		} catch (e) {
			showToast(String(e), 'error');
		} finally {
			isWorking = false;
		}
	}

	/** Check if a rebase is already in progress and load its state. */
	export async function loadActiveRebase() {
		const repo = get(currentRepo);
		if (!repo) return;
		try {
			rebaseState = await getRebaseState(repo);
			if (rebaseState.in_progress) {
				mode = 'progress';
				todoItems = rebaseState.todo_items;
			}
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	async function startRebase() {
		const repo = get(currentRepo);
		if (!repo) return;
		isWorking = true;
		try {
			const msg = await startInteractiveRebase(repo, ontoHash, todoItems);
			showToast(msg || 'Rebase started', 'success');
			// Check if rebase completed or paused
			rebaseState = await getRebaseState(repo);
			if (rebaseState.in_progress) {
				mode = 'progress';
				todoItems = rebaseState.todo_items;
			} else {
				closeDiff();
			}
		} catch (e) {
			showToast(String(e), 'error');
		} finally {
			isWorking = false;
		}
	}

	async function handleContinue() {
		const repo = get(currentRepo);
		if (!repo) return;
		isWorking = true;
		try {
			const msg = await continueRebase(repo);
			showToast(msg || 'Rebase continued', 'success');
			rebaseState = await getRebaseState(repo);
			if (!rebaseState.in_progress) {
				closeDiff();
			} else {
				todoItems = rebaseState.todo_items;
			}
		} catch (e) {
			showToast(String(e), 'error');
		} finally {
			isWorking = false;
		}
	}

	async function handleAbort() {
		const repo = get(currentRepo);
		if (!repo) return;
		if (!confirm('Abort the rebase? All progress will be lost.')) return;
		try {
			const msg = await abortRebase(repo);
			showToast(msg, 'success');
			closeDiff();
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	async function handleSkip() {
		const repo = get(currentRepo);
		if (!repo) return;
		isWorking = true;
		try {
			const msg = await skipRebase(repo);
			showToast(msg || 'Skipped commit', 'success');
			rebaseState = await getRebaseState(repo);
			if (!rebaseState.in_progress) {
				closeDiff();
			} else {
				todoItems = rebaseState.todo_items;
			}
		} catch (e) {
			showToast(String(e), 'error');
		} finally {
			isWorking = false;
		}
	}

	function setAction(index: number, action: string) {
		todoItems = todoItems.map((item, i) =>
			i === index ? { ...item, action } : item
		);
	}

	function moveItem(from: number, to: number) {
		if (to < 0 || to >= todoItems.length) return;
		const items = [...todoItems];
		const [moved] = items.splice(from, 1);
		items.splice(to, 0, moved);
		todoItems = items;
	}

	function handleDragStart(index: number) {
		dragIndex = index;
	}

	function handleDrop(index: number) {
		if (dragIndex !== null && dragIndex !== index) {
			moveItem(dragIndex, index);
		}
		dragIndex = null;
	}

	function onRebaseSetup(e: Event) {
		const detail = (e as CustomEvent).detail;
		if (detail?.hash) {
			setupRebase(detail.hash);
		}
	}

	onMount(() => {
		window.addEventListener('gitaxon-rebase-setup', onRebaseSetup);
		// Check if there's an active rebase
		loadActiveRebase();
	});

	onDestroy(() => {
		window.removeEventListener('gitaxon-rebase-setup', onRebaseSetup);
	});
</script>

<div class="rebase-panel">
	<div class="rb-header">
		<button class="rb-back" onclick={() => closeDiff()}>←</button>
		<div class="rb-title">
			<span class="rb-label">INTERACTIVE REBASE</span>
			{#if mode === 'setup'}
				<span class="rb-sub">onto {ontoHash.slice(0, 7)}</span>
			{:else if rebaseState}
				<span class="rb-sub">Step {rebaseState.current_step}/{rebaseState.total_steps}</span>
			{/if}
		</div>
		<div class="rb-actions">
			{#if mode === 'setup'}
				<button class="rb-btn start" onclick={startRebase} disabled={isWorking || todoItems.length === 0}>
					{isWorking ? 'Starting...' : 'Start Rebase'}
				</button>
			{:else}
				<button class="rb-btn continue-btn" onclick={handleContinue} disabled={isWorking}>Continue</button>
				<button class="rb-btn skip-btn" onclick={handleSkip} disabled={isWorking}>Skip</button>
				<button class="rb-btn abort-btn" onclick={handleAbort}>Abort</button>
			{/if}
		</div>
	</div>

	<div class="rb-list">
		{#if isWorking && todoItems.length === 0}
			<div class="rb-loading">Loading commits...</div>
		{:else if todoItems.length === 0}
			<div class="rb-empty">No commits in range</div>
		{:else}
			<div class="rb-legend">
				<span>Drag to reorder. Change action to squash, fixup, reword, edit, or drop.</span>
			</div>
			{#each todoItems as item, i (item.short_hash + i)}
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<div
					class="rb-item"
					class:dropped={item.action === 'drop'}
					draggable={mode === 'setup'}
					ondragstart={() => handleDragStart(i)}
					ondragover={(e) => e.preventDefault()}
					ondrop={() => handleDrop(i)}
				>
					{#if mode === 'setup'}
						<div class="rb-drag-handle">⠿</div>
						<select
							class="rb-action-select"
							value={item.action}
							onchange={(e) => setAction(i, (e.target as HTMLSelectElement).value)}
						>
							{#each ACTIONS as action}
								<option value={action}>{action}</option>
							{/each}
						</select>
					{:else}
						<span class="rb-action-badge">{item.action}</span>
					{/if}
					<span class="rb-hash">{item.short_hash}</span>
					<span class="rb-message">{item.message}</span>
					{#if mode === 'setup'}
						<div class="rb-move-btns">
							<button class="rb-move" onclick={() => moveItem(i, i - 1)} disabled={i === 0}>▲</button>
							<button class="rb-move" onclick={() => moveItem(i, i + 1)} disabled={i === todoItems.length - 1}>▼</button>
						</div>
					{/if}
				</div>
			{/each}
		{/if}
	</div>
</div>

<style>
	.rebase-panel {
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--bg-primary);
	}

	.rb-header {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 12px;
		border-bottom: 1px solid var(--border);
		flex-shrink: 0;
	}

	.rb-back {
		background: none;
		border: none;
		color: var(--text-secondary);
		cursor: pointer;
		font-size: 16px;
		padding: 4px 8px;
		border-radius: 4px;
	}

	.rb-back:hover { background: var(--bg-secondary); }

	.rb-title {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.rb-label {
		font-size: 10px;
		font-weight: 600;
		letter-spacing: 0.5px;
		color: var(--text-muted);
	}

	.rb-sub {
		font-size: 12px;
		color: var(--accent-blue);
		font-family: monospace;
	}

	.rb-actions { display: flex; gap: 4px; }

	.rb-btn {
		padding: 4px 12px;
		font-size: 11px;
		border: 1px solid var(--border);
		border-radius: 3px;
		cursor: pointer;
		background: var(--bg-secondary);
		color: var(--text-secondary);
	}
	.rb-btn:hover { background: var(--bg-tertiary); color: var(--text-primary); }
	.rb-btn:disabled { opacity: 0.5; cursor: default; }
	.start { background: var(--accent-blue); color: white; border: none; }
	.continue-btn { background: var(--accent-green); color: white; border: none; }
	.skip-btn { background: var(--accent-orange); color: white; border: none; }
	.abort-btn { color: var(--accent-red); border-color: var(--accent-red); }

	.rb-list {
		flex: 1;
		overflow-y: auto;
		padding: 8px 0;
	}

	.rb-loading, .rb-empty {
		padding: 24px;
		text-align: center;
		color: var(--text-muted);
	}

	.rb-legend {
		padding: 4px 16px 8px;
		font-size: 10px;
		color: var(--text-muted);
	}

	.rb-item {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 6px 12px;
		border-bottom: 1px solid var(--border);
		cursor: grab;
	}
	.rb-item:hover { background: var(--bg-secondary); }
	.rb-item.dropped { opacity: 0.4; text-decoration: line-through; }

	.rb-drag-handle {
		color: var(--text-muted);
		font-size: 14px;
		cursor: grab;
		flex-shrink: 0;
	}

	.rb-action-select {
		width: 70px;
		padding: 2px 4px;
		font-size: 11px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 3px;
		color: var(--text-primary);
		flex-shrink: 0;
	}

	.rb-action-badge {
		width: 54px;
		text-align: center;
		padding: 2px 6px;
		font-size: 10px;
		font-weight: 600;
		border-radius: 3px;
		background: var(--bg-tertiary);
		color: var(--text-secondary);
		flex-shrink: 0;
	}

	.rb-hash {
		font-family: monospace;
		font-size: 11px;
		color: var(--accent-orange);
		flex-shrink: 0;
		width: 60px;
	}

	.rb-message {
		font-size: 12px;
		color: var(--text-primary);
		flex: 1;
		min-width: 0;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.rb-move-btns {
		display: flex;
		flex-direction: column;
		gap: 1px;
		flex-shrink: 0;
	}

	.rb-move {
		width: 18px;
		height: 14px;
		border: none;
		background: var(--bg-tertiary);
		color: var(--text-muted);
		cursor: pointer;
		font-size: 8px;
		border-radius: 2px;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.rb-move:hover { background: var(--border); color: var(--text-primary); }
	.rb-move:disabled { opacity: 0.3; cursor: default; }
</style>
