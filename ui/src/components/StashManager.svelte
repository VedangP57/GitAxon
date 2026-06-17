<script lang="ts">
	import { get } from 'svelte/store';
	import { currentRepo, selectStash } from '$lib/store';
	import { showToast } from '$lib/toast';
	import {
		listStashes,
		stashPush,
		stashPop,
		stashApply,
		stashDrop,
		stashBranch,
		type StashEntry
	} from '$lib/tauri';

	let stashes = $state<StashEntry[]>([]);
	let isOpen = $state(true);
	let isLoading = $state(false);
	let showPushModal = $state(false);
	let stashMessage = $state('');
	let branchModalStash = $state<StashEntry | null>(null);
	let newBranchName = $state('');
	let ctxMenu = $state<{ x: number; y: number; stash: StashEntry } | null>(null);

	async function loadStashes() {
		const repo = $currentRepo;
		if (!repo) return;
		isLoading = true;
		try {
			stashes = await listStashes(repo);
		} catch {
			stashes = [];
		} finally {
			isLoading = false;
		}
	}

	async function handlePush() {
		const repo = $currentRepo;
		if (!repo) return;
		try {
			const msg = await stashPush(repo, stashMessage);
			showToast(msg || 'Changes stashed', 'success');
			stashMessage = '';
			showPushModal = false;
			await loadStashes();
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	async function handlePop(stash: StashEntry) {
		ctxMenu = null;
		const repo = $currentRepo;
		if (!repo) return;
		try {
			const msg = await stashPop(repo, stash.index);
			showToast(msg, 'success');
			await loadStashes();
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	async function handleApply(stash: StashEntry) {
		ctxMenu = null;
		const repo = $currentRepo;
		if (!repo) return;
		try {
			const msg = await stashApply(repo, stash.index);
			showToast(msg, 'success');
			await loadStashes();
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	async function handleDrop(stash: StashEntry) {
		ctxMenu = null;
		if (!confirm(`Drop ${stash.name}? This cannot be undone.`)) return;
		const repo = $currentRepo;
		if (!repo) return;
		try {
			const msg = await stashDrop(repo, stash.index);
			showToast(msg, 'success');
			await loadStashes();
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	async function handleBranch() {
		if (!branchModalStash || !newBranchName.trim()) return;
		const repo = $currentRepo;
		if (!repo) return;
		try {
			const msg = await stashBranch(repo, branchModalStash.index, newBranchName.trim());
			showToast(msg, 'success');
			branchModalStash = null;
			newBranchName = '';
			await loadStashes();
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	$effect(() => {
		if ($currentRepo) {
			loadStashes();
		} else {
			stashes = [];
		}
	});
</script>

<div class="stash-section">
	<div class="section-header">
		<button class="header-toggle" onclick={() => (isOpen = !isOpen)}>
			<span class="chevron">{isOpen ? '▼' : '▶'}</span>
			<span class="header-label">STASHES</span>
			{#if stashes.length > 0}
				<span class="count-badge">{stashes.length}</span>
			{/if}
		</button>
		<button class="stash-new-btn" title="Create new stash" onclick={() => (showPushModal = true)}>+</button>
	</div>

	{#if isOpen}
		{#if isLoading}
			<div class="stash-loading">Loading...</div>
		{:else if stashes.length === 0}
			<div class="stash-empty">No stashes</div>
		{:else}
			{#each stashes as stash (stash.index)}
				<div
					class="stash-row"
					role="button"
					tabindex="0"
					onclick={() => selectStash(stash.index)}
					onkeydown={(e) => { if (e.key === 'Enter') selectStash(stash.index); }}
					oncontextmenu={(e) => {
						e.preventDefault();
						ctxMenu = { x: e.clientX, y: e.clientY, stash };
					}}
				>
					<div class="stash-icon">S</div>
					<div class="stash-info">
						<span class="stash-name">{stash.name}</span>
						<span class="stash-msg">{stash.message}</span>
						<span class="stash-branch">on {stash.branch}</span>
					</div>
					<div class="stash-actions">
						<button
							class="stash-action-btn"
							title="Pop (apply and remove)"
							onclick={() => handlePop(stash)}
						>
							↑
						</button>
						<button
							class="stash-action-btn"
							title="Apply (keep stash)"
							onclick={() => handleApply(stash)}
						>
							↙
						</button>
						<button class="stash-action-btn danger" title="Drop stash" onclick={() => handleDrop(stash)}>
							×
						</button>
					</div>
				</div>
			{/each}
		{/if}
	{/if}

	{#if showPushModal}
		<div class="modal-overlay" role="none" onclick={() => (showPushModal = false)}>
			<div class="modal" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
				<h3>Create Stash</h3>
				<input
					class="modal-input"
					type="text"
					placeholder="Optional message..."
					bind:value={stashMessage}
					onkeydown={(e) => e.key === 'Enter' && handlePush()}
				/>
				<div class="modal-actions">
					<button class="btn-cancel" onclick={() => (showPushModal = false)}>Cancel</button>
					<button class="btn-primary" onclick={handlePush}>Stash Changes</button>
				</div>
			</div>
		</div>
	{/if}

	{#if branchModalStash}
		<div class="modal-overlay" role="none" onclick={() => (branchModalStash = null)}>
			<div class="modal" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
				<h3>Create Branch from Stash</h3>
				<p class="modal-sub">{branchModalStash.name}</p>
				<input
					class="modal-input"
					type="text"
					placeholder="New branch name..."
					bind:value={newBranchName}
					onkeydown={(e) => e.key === 'Enter' && handleBranch()}
				/>
				<div class="modal-actions">
					<button class="btn-cancel" onclick={() => (branchModalStash = null)}>Cancel</button>
					<button class="btn-primary" onclick={handleBranch}>Create Branch</button>
				</div>
			</div>
		</div>
	{/if}

	{#if ctxMenu}
		<div class="ctx-backdrop" role="none" onclick={() => (ctxMenu = null)}></div>
		<div class="ctx-menu" style={`top:${ctxMenu.y}px; left:${ctxMenu.x}px`}>
			<button class="ctx-item" onclick={() => { selectStash(ctxMenu!.stash.index); ctxMenu = null; }}>
				Preview stash
			</button>
			<div class="ctx-divider"></div>
			<button class="ctx-item" onclick={() => handlePop(ctxMenu!.stash)}>
				↑ Pop (apply and remove)
			</button>
			<button class="ctx-item" onclick={() => handleApply(ctxMenu!.stash)}>
				↙ Apply (keep stash)
			</button>
			<div class="ctx-divider"></div>
			<button
				class="ctx-item"
				onclick={() => {
					branchModalStash = ctxMenu!.stash;
					ctxMenu = null;
				}}
			>
				Create branch from stash
			</button>
			<div class="ctx-divider"></div>
			<button class="ctx-item ctx-danger" onclick={() => handleDrop(ctxMenu!.stash)}>× Drop stash</button>
		</div>
	{/if}
</div>

<style>
	.stash-section {
		border-top: 1px solid var(--border);
	}

	.section-header {
		display: flex;
		align-items: center;
		height: 32px;
		padding: 0 8px;
		gap: 4px;
	}

	.header-toggle {
		flex: 1;
		display: flex;
		align-items: center;
		gap: 6px;
		background: none;
		border: none;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 10px;
		font-weight: 600;
		letter-spacing: 0.5px;
		text-transform: uppercase;
		padding: 0;
	}

	.header-toggle:hover {
		color: var(--text-primary);
	}

	.chevron {
		font-size: 8px;
	}

	.count-badge {
		background: var(--bg-tertiary);
		color: var(--text-muted);
		border-radius: 8px;
		padding: 0 5px;
		font-size: 10px;
		margin-left: auto;
	}

	.stash-new-btn {
		width: 20px;
		height: 20px;
		border-radius: 4px;
		border: none;
		background: transparent;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 16px;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.stash-new-btn:hover {
		background: var(--bg-tertiary);
		color: var(--accent-green);
	}

	.stash-empty,
	.stash-loading {
		padding: 8px 16px;
		font-size: 12px;
		color: var(--text-muted);
		font-style: italic;
	}

	.stash-row {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 5px 8px 5px 16px;
		cursor: pointer;
	}

	.stash-row:hover {
		background: var(--bg-secondary);
	}

	.stash-icon {
		font-size: 12px;
		flex-shrink: 0;
		color: var(--accent-blue);
		font-weight: 700;
	}

	.stash-info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 1px;
	}

	.stash-name {
		font-size: 12px;
		color: var(--accent-blue);
		font-family: monospace;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.stash-msg {
		font-size: 11px;
		color: var(--text-secondary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.stash-branch {
		font-size: 10px;
		color: var(--text-muted);
	}

	.stash-actions {
		display: flex;
		gap: 2px;
		opacity: 0;
		transition: opacity 0.1s;
	}

	.stash-row:hover .stash-actions {
		opacity: 1;
	}

	.stash-action-btn {
		width: 22px;
		height: 22px;
		border: none;
		background: var(--bg-tertiary);
		color: var(--text-secondary);
		border-radius: 3px;
		cursor: pointer;
		font-size: 11px;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.stash-action-btn:hover {
		background: var(--border);
		color: var(--text-primary);
	}

	.stash-action-btn.danger:hover {
		background: rgba(248, 81, 73, 0.15);
		color: var(--accent-red);
	}

	.modal-overlay {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.5);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
	}

	.modal {
		background: var(--bg-secondary);
		border: 1px solid var(--border);
		border-radius: 8px;
		padding: 20px;
		width: 320px;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.modal h3 {
		font-size: 14px;
		color: var(--text-primary);
		margin: 0;
	}

	.modal-sub {
		font-size: 12px;
		color: var(--text-muted);
		margin: 0;
	}

	.modal-input {
		background: var(--bg-primary);
		border: 1px solid var(--border);
		border-radius: 4px;
		padding: 8px 10px;
		color: var(--text-primary);
		font-size: 13px;
		outline: none;
	}

	.modal-input:focus {
		border-color: var(--accent-blue);
	}

	.modal-actions {
		display: flex;
		gap: 8px;
		justify-content: flex-end;
	}

	.btn-cancel {
		padding: 6px 14px;
		border: 1px solid var(--border);
		background: transparent;
		color: var(--text-secondary);
		border-radius: 4px;
		cursor: pointer;
		font-size: 12px;
	}

	.btn-primary {
		padding: 6px 14px;
		border: none;
		background: var(--accent-green);
		color: white;
		border-radius: 4px;
		cursor: pointer;
		font-size: 12px;
		font-weight: 600;
	}

	.ctx-backdrop {
		position: fixed;
		inset: 0;
		z-index: 998;
	}

	.ctx-menu {
		position: fixed;
		z-index: 999;
		background: var(--bg-secondary);
		border: 1px solid var(--border);
		border-radius: 6px;
		padding: 4px;
		min-width: 200px;
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
	}

	.ctx-item {
		display: block;
		width: 100%;
		padding: 6px 10px;
		border: none;
		background: transparent;
		color: var(--text-primary);
		cursor: pointer;
		border-radius: 3px;
		font-size: 12px;
		text-align: left;
	}

	.ctx-item:hover {
		background: var(--bg-tertiary);
	}

	.ctx-item.ctx-danger {
		color: var(--accent-red);
	}

	.ctx-item.ctx-danger:hover {
		background: rgba(248, 81, 73, 0.1);
	}

	.ctx-divider {
		height: 1px;
		background: var(--border);
		margin: 2px 0;
	}
</style>
