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
