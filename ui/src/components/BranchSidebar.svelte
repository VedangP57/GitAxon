<script lang="ts">
	import { branches, currentRepo, loadRepo } from '$lib/store';
	import {
		deleteBranch,
		renameBranch,
		mergeBranch
	} from '$lib/tauri';
	import { invoke } from '@tauri-apps/api/core';
	import { showToast } from '$lib/toast';
	import type { BranchInfo } from '$lib/types';

	let searchQuery = $state('');
	let localOpen = $state(true);
	let remotesOpen = $state(true);
	let tagsOpen = $state(true);
	let contextMenu = $state<{
		x: number;
		y: number;
		branch: BranchInfo;
		isLocal: boolean;
	} | null>(null);
	let createBranchName = $state('');
	let createBranchFrom = $state('HEAD');
	let showCreateForm = $state(false);

	const branchList = $derived($branches);
	const repoPath = $derived($currentRepo);

	const filteredLocal = $derived(
		branchList.filter(
			(b) =>
				!b.isRemote &&
				(searchQuery === '' || b.name.toLowerCase().includes(searchQuery.toLowerCase()))
		)
	);

	const filteredRemote = $derived(
		branchList.filter(
			(b) =>
				b.isRemote &&
				(searchQuery === '' || b.name.toLowerCase().includes(searchQuery.toLowerCase()))
		)
	);

	const remoteGroups = $derived.by(() => {
		const groups = new Map<string, BranchInfo[]>();
		for (const b of filteredRemote) {
			const remote = b.name.split('/')[0] ?? 'origin';
			if (!groups.has(remote)) groups.set(remote, []);
			groups.get(remote)!.push(b);
		}
		return groups;
	});

	const headBranch = $derived(branchList.find((b) => b.isHead));

	function shortHash(hash: string): string {
		return hash.slice(0, 7);
	}

	async function onCheckout(b: BranchInfo) {
		if (b.isRemote) return;
		if (!repoPath) return;
		try {
			await invoke('checkout_branch', { repoPath, name: b.name });
			await loadRepo(repoPath);
			showToast(`Switched to branch ${b.name}`, 'success');
		} catch (err) {
			const msg = err instanceof Error ? err.message : String(err);
			if (msg.toLowerCase().includes('uncommitted changes')) {
				showToast(
					'Cannot checkout: you have uncommitted changes. Stage or stash them first.',
					'error'
				);
			} else {
				showToast(msg, 'error');
			}
		}
	}

	function showContextMenu(e: MouseEvent, branch: BranchInfo, isLocal: boolean) {
		e.preventDefault();
		e.stopPropagation();
		contextMenu = { x: e.clientX, y: e.clientY, branch, isLocal };
	}

	function hideContextMenu() {
		contextMenu = null;
	}

	async function handleRename() {
		if (!contextMenu || !repoPath || !contextMenu.isLocal) return;
		const newName = prompt('Rename branch to:', contextMenu.branch.name);
		if (!newName || newName === contextMenu.branch.name) {
			hideContextMenu();
			return;
		}
		try {
			await renameBranch(repoPath, contextMenu.branch.name, newName);
			await loadRepo(repoPath);
		} catch (err) {
			alert(err instanceof Error ? err.message : String(err));
		}
		hideContextMenu();
	}

	async function handleDelete() {
		if (!contextMenu || !repoPath || !contextMenu.isLocal) return;
		if (!confirm(`Delete branch "${contextMenu.branch.name}"?`)) {
			hideContextMenu();
			return;
		}
		try {
			await deleteBranch(repoPath, contextMenu.branch.name, false);
			await loadRepo(repoPath);
		} catch (err) {
			alert(err instanceof Error ? err.message : String(err));
		}
		hideContextMenu();
	}

	async function handleMerge() {
		if (!contextMenu || !repoPath || !contextMenu.isLocal) return;
		if (contextMenu.branch.isHead) {
			alert('Cannot merge current branch into itself');
			hideContextMenu();
			return;
		}
		try {
			await mergeBranch(repoPath, contextMenu.branch.name);
			await loadRepo(repoPath);
		} catch (err) {
			alert(err instanceof Error ? err.message : String(err));
		}
		hideContextMenu();
	}

	function handleCopyName() {
		if (!contextMenu) return;
		navigator.clipboard.writeText(contextMenu.branch.name);
		hideContextMenu();
	}

	function openCreateForm(fromRef: string = 'HEAD') {
		createBranchFrom = fromRef;
		createBranchName = '';
		showCreateForm = true;
	}

	async function submitCreateBranch() {
		if (!repoPath || !createBranchName.trim()) {
			showCreateForm = false;
			return;
		}
		const newBranchName = createBranchName.trim();
		const fromBranch = createBranchFrom.trim() || 'HEAD';
		try {
			await invoke('create_branch', {
				repoPath,
				name: newBranchName,
				fromRef: fromBranch
			});
			await invoke('checkout_branch', { repoPath, name: newBranchName });
			await loadRepo(repoPath);
			showToast(`Branch ${newBranchName} created and checked out`, 'success');
		} catch (err) {
			showToast(err instanceof Error ? err.message : String(err), 'error');
		}
		showCreateForm = false;
	}

	async function createFromRemote(remoteBranch: BranchInfo) {
		createBranchFrom = remoteBranch.name;
		createBranchName = remoteBranch.name.split('/').slice(1).join('/') || remoteBranch.name;
		showCreateForm = true;
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			hideContextMenu();
			showCreateForm = false;
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_no_noninteractive_element_interactions -->
<div class="sidebar" onclick={hideContextMenu} role="region" aria-label="Branch sidebar">
	<div class="search-wrap">
		<input
			type="text"
			class="search-input"
			placeholder="Filter branches..."
			bind:value={searchQuery}
		/>
	</div>

	<!-- LOCAL BRANCHES -->
	<div class="section">
		<div class="section-header">
			<button
				type="button"
				class="header-toggle"
				onclick={() => (localOpen = !localOpen)}
			>
				<span class="chevron">{localOpen ? '▼' : '▶'}</span>
				<span>LOCAL</span>
			</button>
			<button
				class="add-btn"
				onclick={() => openCreateForm('HEAD')}
				title="Create branch"
				type="button"
			>
				+
			</button>
		</div>
		{#if localOpen}
			<div class="branch-list">
				{#if filteredLocal.length === 0}
					<div class="empty">
						{searchQuery ? `No branches match '${searchQuery}'` : 'No local branches'}
					</div>
				{:else}
				{#each filteredLocal as branch (branch.name)}
					<div
						class="branch-row"
						class:current={branch.isHead}
						onclick={() => onCheckout(branch)}
						oncontextmenu={(e) => showContextMenu(e, branch, true)}
						role="button"
						tabindex="0"
						onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), onCheckout(branch))}
					>
						{#if branch.isHead}
							<span class="head-dot" title="Current branch"></span>
						{/if}
						<span class="branch-icon">○</span>
						<span class="branch-name">{branch.name}</span>
						<span class="branch-hash">{shortHash(branch.tipHash)}</span>
						<button
							class="branch-add-btn"
							title="Create branch from {branch.name}"
							type="button"
							onclick={(e) => {
								e.stopPropagation();
								openCreateForm(branch.name);
							}}
						>
							+
						</button>
					</div>
				{/each}
				{/if}
			</div>
		{/if}
	</div>

	<!-- REMOTE BRANCHES -->
	<div class="section">
		<button
			class="section-header"
			onclick={() => (remotesOpen = !remotesOpen)}
			type="button"
		>
			<span class="chevron">{remotesOpen ? '▼' : '▶'}</span>
			<span>REMOTES (origin)</span>
		</button>
		{#if remotesOpen}
			{#each Array.from(remoteGroups.entries()) as [remoteName, branches]}
				<div class="remote-group">
					<div class="remote-label">{remoteName}</div>
					<div class="branch-list">
						{#each branches as branch (branch.name)}
							<div
								class="branch-row remote"
								title="Create local branch from remote?"
								onclick={() => createFromRemote(branch)}
								onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), createFromRemote(branch))}
								oncontextmenu={(e) => showContextMenu(e, branch, false)}
								role="button"
								tabindex="0"
							>
								<span class="branch-icon">○</span>
								<span class="branch-name">{branch.name.replace(remoteName + '/', '')}</span>
								<span class="branch-hash">{shortHash(branch.tipHash)}</span>
							</div>
						{/each}
					</div>
				</div>
			{/each}
			{#if filteredRemote.length === 0}
				<div class="empty">No remote branches</div>
			{/if}
		{/if}
	</div>

	<!-- TAGS -->
	<div class="section">
		<button
			class="section-header"
			onclick={() => (tagsOpen = !tagsOpen)}
			type="button"
		>
			<span class="chevron">{tagsOpen ? '▼' : '▶'}</span>
			<span>TAGS</span>
		</button>
		{#if tagsOpen}
			<div class="empty">No tags</div>
		{/if}
	</div>
</div>

{#if showCreateForm}
	<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
	<div class="modal-overlay" onclick={() => (showCreateForm = false)} role="presentation">
		<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
		<div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-label="Create branch" tabindex="-1">
			<h3>Create Branch</h3>
			<label>
				Branch name
				<input
					type="text"
					bind:value={createBranchName}
					placeholder="branch-name"
					onkeydown={(e) => e.key === 'Enter' && submitCreateBranch()}
				/>
			</label>
			<label>
				From
				<input
					type="text"
					bind:value={createBranchFrom}
					placeholder="HEAD or branch name"
				/>
			</label>
			<div class="modal-actions">
				<button type="button" onclick={() => (showCreateForm = false)}>Cancel</button>
				<button type="button" onclick={submitCreateBranch}>Create</button>
			</div>
		</div>
	</div>
{/if}

{#if contextMenu}
	<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
	<div class="context-menu-backdrop" onclick={hideContextMenu} role="presentation"></div>
	<div
		class="context-menu"
		style="left: {contextMenu.x}px; top: {contextMenu.y}px;"
		role="menu"
	>
		{#if contextMenu.isLocal}
			<button type="button" onclick={handleRename} role="menuitem">Rename</button>
			<button type="button" onclick={handleDelete} role="menuitem">Delete</button>
			{#if !contextMenu.branch.isHead && headBranch}
				<button type="button" onclick={handleMerge} role="menuitem">Merge into {headBranch.name}</button>
			{/if}
		{/if}
		<button type="button" onclick={handleCopyName} role="menuitem">Copy branch name</button>
	</div>
{/if}

<style>
	.sidebar {
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--bg-secondary);
		color: var(--text-primary);
		overflow: hidden;
	}

	.search-wrap {
		padding: 8px 12px;
		flex-shrink: 0;
	}

	.search-input {
		width: 100%;
		padding: 6px 10px;
		font-size: 12px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text-primary);
	}
	.search-input::placeholder {
		color: var(--text-muted);
	}

	.section {
		flex-shrink: 0;
	}

	.section-header {
		display: flex;
		align-items: center;
		gap: 6px;
		width: 100%;
		padding: 8px 12px;
		background: none;
		border: none;
		color: var(--text-muted);
		font-size: 11px;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		cursor: pointer;
		text-align: left;
	}

	.section-header:hover {
		color: var(--text-secondary);
	}

	.header-toggle {
		flex: 1;
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 0;
		background: none;
		border: none;
		color: inherit;
		font: inherit;
		letter-spacing: inherit;
		text-transform: inherit;
		text-align: left;
		cursor: pointer;
	}

	.chevron {
		font-size: 10px;
		opacity: 0.8;
	}

	.add-btn {
		margin-left: auto;
		padding: 2px 8px;
		font-size: 14px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 4px;
		color: var(--text-primary);
		cursor: pointer;
	}
	.add-btn:hover {
		background: var(--border);
	}

	.branch-list {
		overflow-y: auto;
	}

	.branch-row {
		display: flex;
		align-items: center;
		gap: 8px;
		height: 28px;
		padding: 0 12px;
		font-size: 13px;
		cursor: pointer;
		transition: background-color 80ms ease;
	}

	.branch-row:hover {
		background: var(--bg-tertiary);
	}

	.branch-row.current {
		background: #1f2d1f;
		color: var(--accent-green);
		font-weight: 600;
	}

	.branch-row.remote {
		color: #8b949e;
	}

	.head-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--accent-green);
		flex-shrink: 0;
	}

	.branch-icon {
		font-size: 12px;
		opacity: 0.7;
		flex-shrink: 0;
	}

	.branch-name {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.branch-hash {
		font-size: 11px;
		color: var(--text-muted);
		font-family: ui-monospace, monospace;
		flex-shrink: 0;
	}

	.branch-add-btn {
		margin-left: auto;
		opacity: 0;
		padding: 2px 6px;
		font-size: 12px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 4px;
		color: var(--text-primary);
		cursor: pointer;
		flex-shrink: 0;
	}
	.branch-row:hover .branch-add-btn {
		opacity: 1;
	}
	.branch-add-btn:hover {
		background: var(--accent-green);
		color: #0d1117;
		border-color: var(--accent-green);
	}

	.remote-group {
		padding-left: 12px;
	}

	.remote-label {
		padding: 4px 12px;
		font-size: 10px;
		color: var(--text-muted);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.empty {
		padding: 8px 12px;
		font-size: 12px;
		color: var(--text-muted);
	}

	.context-menu-backdrop {
		position: fixed;
		inset: 0;
		z-index: 999;
		background: transparent;
	}

	.context-menu {
		position: fixed;
		z-index: 1000;
		min-width: 180px;
		padding: 4px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 6px;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
	}

	.context-menu button {
		display: block;
		width: 100%;
		padding: 8px 12px;
		background: none;
		border: none;
		color: var(--text-primary);
		font-size: 13px;
		text-align: left;
		cursor: pointer;
		border-radius: 4px;
	}

	.context-menu button:hover {
		background: var(--bg-secondary);
	}

	.modal-overlay {
		position: fixed;
		inset: 0;
		z-index: 1001;
		background: rgba(0, 0, 0, 0.5);
		display: flex;
		align-items: center;
		justify-content: center;
		animation: modal-fade-in 0.15s ease;
	}
	@keyframes modal-fade-in {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}

	.modal {
		background: var(--bg-secondary);
		border: 1px solid var(--border);
		border-radius: 8px;
		padding: 20px;
		min-width: 320px;
	}

	.modal h3 {
		margin: 0 0 16px 0;
		font-size: 16px;
	}

	.modal label {
		display: block;
		margin-bottom: 12px;
		font-size: 12px;
		color: var(--text-secondary);
	}

	.modal input {
		display: block;
		width: 100%;
		margin-top: 4px;
		padding: 8px 12px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text-primary);
		font-size: 13px;
	}

	.modal-actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
		margin-top: 16px;
	}

	.modal-actions button {
		padding: 8px 16px;
		border-radius: 6px;
		font-size: 13px;
		cursor: pointer;
		border: 1px solid var(--border);
	}

	.modal-actions button:first-child {
		background: var(--bg-tertiary);
		color: var(--text-primary);
	}

	.modal-actions button:last-child {
		background: var(--accent-green);
		color: white;
		border-color: var(--accent-green);
	}
</style>
