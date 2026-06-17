<script lang="ts">
	import { openTabs, activeTabIndex, switchTab, closeTab, loadRepo } from '$lib/store';
	import { open } from '@tauri-apps/plugin-dialog';

	let tabs = $state<{ path: string; name: string }[]>([]);
	let activeIdx = $state(0);

	$effect(() => {
		const unsub1 = openTabs.subscribe((v) => (tabs = v));
		const unsub2 = activeTabIndex.subscribe((v) => (activeIdx = v));
		return () => { unsub1(); unsub2(); };
	});

	async function openNewRepo() {
		try {
			const selected = await open({ directory: true, multiple: false });
			if (selected && typeof selected === 'string') {
				await loadRepo(selected);
			}
		} catch {
			// silently ignore dialog cancel
		}
	}
</script>

<div class="tab-bar">
	{#each tabs as tab, i (tab.path)}
		<div
			class="tab"
			class:active={i === activeIdx}
			onclick={() => switchTab(i)}
			onkeydown={(e) => e.key === 'Enter' && switchTab(i)}
			role="tab"
			tabindex="0"
			title={tab.path}
		>
			<span class="tab-name">{tab.name}</span>
			{#if tabs.length > 1}
				<button
					class="tab-close"
					onclick={(e) => { e.stopPropagation(); closeTab(i); }}
					title="Close tab"
				>×</button>
			{/if}
		</div>
	{/each}
	<button class="tab-add" onclick={openNewRepo} title="Open repository">+</button>
</div>

<style>
	.tab-bar {
		display: flex;
		align-items: stretch;
		height: 30px;
		background: var(--bg-primary);
		border-bottom: 1px solid var(--border);
		overflow-x: auto;
		flex-shrink: 0;
	}

	.tab-bar::-webkit-scrollbar {
		height: 0;
	}

	.tab {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 0 12px;
		border: none;
		border-right: 1px solid var(--border);
		background: var(--bg-primary);
		color: var(--text-muted);
		cursor: pointer;
		font-size: 11px;
		white-space: nowrap;
		min-width: 0;
		max-width: 180px;
		transition: background 0.1s;
	}

	.tab:hover {
		background: var(--bg-secondary);
		color: var(--text-primary);
	}

	.tab.active {
		background: var(--bg-secondary);
		color: var(--text-primary);
		border-bottom: 2px solid var(--accent-blue);
	}

	.tab-name {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.tab-close {
		width: 16px;
		height: 16px;
		border: none;
		background: transparent;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 12px;
		border-radius: 3px;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		opacity: 0;
		transition: opacity 0.1s;
	}

	.tab:hover .tab-close {
		opacity: 1;
	}

	.tab-close:hover {
		background: var(--bg-tertiary);
		color: var(--accent-red);
	}

	.tab-add {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 30px;
		height: 100%;
		border: none;
		border-right: 1px solid var(--border);
		background: transparent;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 16px;
		flex-shrink: 0;
		transition: background 0.1s, color 0.1s;
	}

	.tab-add:hover {
		background: var(--bg-secondary);
		color: var(--text-primary);
	}
</style>
