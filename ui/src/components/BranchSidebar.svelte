<script lang="ts">
	import { branches, currentRepo, loadRepo, openCreateBranchForm, createBranchFromHash } from '$lib/store';
	import {
		deleteBranch,
		renameBranch,
		mergeBranch
	} from '$lib/tauri';
	import { invoke } from '@tauri-apps/api/core';
	import { showToast } from '$lib/toast';
	import type { BranchInfo } from '$lib/types';
	import StashManager from './StashManager.svelte';

	let searchQuery = $state('');
	let localOpen = $state(true);
	let remotesOpen = $state(true);
	let tagsOpen = $state(false);
	let cloudPatchesOpen = $state(false);
	let prOpen = $state(false);
	let issuesOpen = $state(false);
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

	const repoName = $derived(
		$currentRepo ? $currentRepo.split(/[/\\]/).filter(Boolean).pop() ?? 'Repository' : 'Repository'
	);

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

	$effect(() => {
		if ($openCreateBranchForm) {
			showCreateForm = true;
			openCreateBranchForm.set(false);
		}
	});

	$effect(() => {
		if ($createBranchFromHash) {
			openCreateForm($createBranchFromHash);
			createBranchFromHash.set(null);
		}
	});

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
				showToast('Cannot checkout: you have uncommitted changes. Stage or stash them first.', 'error');
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
		if (!newName || newName === contextMenu.branch.name) { hideContextMenu(); return; }
		try {
			await renameBranch(repoPath, contextMenu.branch.name, newName);
			await loadRepo(repoPath);
			showToast(`Branch renamed to ${newName}`, 'success');
		} catch (err) {
			showToast(err instanceof Error ? err.message : String(err), 'error');
		}
		hideContextMenu();
	}

	async function handleDelete() {
		if (!contextMenu || !repoPath || !contextMenu.isLocal) return;
		if (!confirm(`Delete branch "${contextMenu.branch.name}"?`)) { hideContextMenu(); return; }
		try {
			await deleteBranch(repoPath, contextMenu.branch.name, false);
			await loadRepo(repoPath);
			showToast(`Branch ${contextMenu.branch.name} deleted`, 'success');
		} catch (err) {
			showToast(err instanceof Error ? err.message : String(err), 'error');
		}
		hideContextMenu();
	}

	async function handleMerge() {
		if (!contextMenu || !repoPath || !contextMenu.isLocal) return;
		if (contextMenu.branch.isHead) {
			showToast('Cannot merge current branch into itself', 'error');
			hideContextMenu();
			return;
		}
		try {
			await mergeBranch(repoPath, contextMenu.branch.name);
			await loadRepo(repoPath);
			showToast(`Merged ${contextMenu.branch.name} into current branch`, 'success');
		} catch (err) {
			showToast(err instanceof Error ? err.message : String(err), 'error');
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
		if (!repoPath || !createBranchName.trim()) { showCreateForm = false; return; }
		const newBranchName = createBranchName.trim();
		const fromBranch = createBranchFrom.trim() || 'HEAD';
		try {
			await invoke('create_branch', { repoPath, name: newBranchName, fromRef: fromBranch });
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
		if (e.key === 'Escape') { hideContextMenu(); showCreateForm = false; }
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="sidebar" role="region" aria-label="Branch sidebar">

	<!-- Repo info header -->
	<div class="repo-header">
		<div class="repo-row">
			<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="row-icon"><path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>
			<span class="repo-label">repository</span>
			<span class="repo-name">{repoName}</span>
		</div>
		{#if headBranch}
			<div class="repo-row">
				<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="row-icon"><line x1="6" y1="3" x2="6" y2="15"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M18 9a9 9 0 0 1-9 9"/></svg>
				<span class="repo-label">branch</span>
				<span class="branch-current-name">{headBranch.name}</span>
			</div>
		{/if}
	</div>

	<!-- Scrollable section list -->
	<div class="section-list">

		<!-- ▼ LOCAL -->
		<div class="section">
			<div class="section-header-row">
				<button class="section-toggle" onclick={() => (localOpen = !localOpen)}>
					<span class="chevron">{localOpen ? '▼' : '▶'}</span>
					<span class="section-label">LOCAL</span>
					<span class="count-pill">{filteredLocal.length}</span>
				</button>
				<button class="section-add" onclick={() => openCreateForm('HEAD')} title="New branch">+</button>
			</div>
			{#if localOpen}
				<div class="search-wrap">
					<input
						type="text"
						class="search-input"
						placeholder="Filter branches..."
						bind:value={searchQuery}
					/>
				</div>
				<div class="branch-list">
					{#if filteredLocal.length === 0}
						<div class="empty">{searchQuery ? `No match for '${searchQuery}'` : 'No local branches'}</div>
					{:else}
						{#each filteredLocal as branch (branch.name)}
							<div
								class="branch-row"
								class:is-head={branch.isHead}
								onclick={() => onCheckout(branch)}
								oncontextmenu={(e) => showContextMenu(e, branch, true)}
								role="button"
								tabindex="0"
								onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), onCheckout(branch))}
							>
								{#if branch.isHead}
									<span class="head-dot" title="Current branch"></span>
								{:else}
									<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="branch-icon"><line x1="6" y1="3" x2="6" y2="15"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M18 9a9 9 0 0 1-9 9"/></svg>
								{/if}
								<span class="branch-name" title={branch.name}>{branch.name}</span>
								<span class="branch-hash">{shortHash(branch.tipHash)}</span>
								<button
									class="checkout-btn"
									title="Create branch from {branch.name}"
									type="button"
									onclick={(e) => { e.stopPropagation(); openCreateForm(branch.name); }}
								>+</button>
							</div>
						{/each}
					{/if}
				</div>
			{/if}
		</div>

		<!-- ▼ REMOTE -->
		<div class="section">
			<button class="section-header-row full-btn" onclick={() => (remotesOpen = !remotesOpen)}>
				<span class="chevron">{remotesOpen ? '▼' : '▶'}</span>
				<span class="section-label">REMOTE</span>
				<span class="count-pill">{filteredRemote.length}</span>
			</button>
			{#if remotesOpen}
				{#each Array.from(remoteGroups.entries()) as [remoteName, remoteBranches]}
					<div class="remote-group">
						<div class="remote-label">{remoteName}</div>
						{#each remoteBranches as branch (branch.name)}
							<div
								class="branch-row remote"
								title="Create local branch from remote?"
								onclick={() => createFromRemote(branch)}
								onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), createFromRemote(branch))}
								oncontextmenu={(e) => showContextMenu(e, branch, false)}
								role="button"
								tabindex="0"
							>
								<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="branch-icon"><line x1="6" y1="3" x2="6" y2="15"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M18 9a9 9 0 0 1-9 9"/></svg>
								<span class="branch-name" title={branch.name.replace(remoteName + '/', '')}
									>{branch.name.replace(remoteName + '/', '')}</span
								>
								<span class="branch-hash">{shortHash(branch.tipHash)}</span>
							</div>
						{/each}
					</div>
				{/each}
				{#if filteredRemote.length === 0}
					<div class="empty">No remote branches</div>
				{/if}
			{/if}
		</div>

		<!-- ▼ CLOUD PATCHES -->
		<div class="section">
			<button class="section-header-row full-btn" onclick={() => (cloudPatchesOpen = !cloudPatchesOpen)}>
				<span class="chevron">{cloudPatchesOpen ? '▼' : '▶'}</span>
				<span class="section-label">CLOUD PATCHES</span>
				<span class="count-pill">0</span>
			</button>
		</div>

		<!-- ▼ PULL REQUESTS -->
		<div class="section">
			<button class="section-header-row full-btn" onclick={() => (prOpen = !prOpen)}>
				<span class="chevron">{prOpen ? '▼' : '▶'}</span>
				<span class="section-label">PULL REQUESTS</span>
				<span class="count-pill">0</span>
			</button>
		</div>

		<!-- ▼ ISSUES -->
		<div class="section">
			<button class="section-header-row full-btn" onclick={() => (issuesOpen = !issuesOpen)}>
				<span class="chevron">{issuesOpen ? '▼' : '▶'}</span>
				<span class="section-label">ISSUES</span>
				<span class="count-pill">0</span>
			</button>
		</div>

		<!-- ▼ TAGS -->
		<div class="section">
			<button class="section-header-row full-btn" onclick={() => (tagsOpen = !tagsOpen)}>
				<span class="chevron">{tagsOpen ? '▼' : '▶'}</span>
				<span class="section-label">TAGS</span>
				<span class="count-pill">0</span>
			</button>
			{#if tagsOpen}
				<div class="empty">No tags</div>
			{/if}
		</div>

		<StashManager />

	</div>

	<!-- Bottom action icons -->
	<div class="sidebar-bottom">
		<span class="app-brand-name">GitAxon</span>
		<div class="bottom-actions">
			<button class="bottom-icon-btn" title="Notifications">
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 0 1-3.46 0"/></svg>
			</button>
			<button class="bottom-icon-btn" title="Settings">
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
			</button>
		</div>
	</div>
</div>

<!-- Create branch modal -->
{#if showCreateForm}
	<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
	<div class="modal-overlay" onclick={() => (showCreateForm = false)} role="presentation">
		<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
		<div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-label="Create branch" tabindex="-1">
			<h3>Create Branch</h3>
			<label>
				Branch name
				<input type="text" bind:value={createBranchName} placeholder="branch-name" onkeydown={(e) => e.key === 'Enter' && submitCreateBranch()} />
			</label>
			<label>
				From
				<input type="text" bind:value={createBranchFrom} placeholder="HEAD or branch name" />
			</label>
			<div class="modal-actions">
				<button type="button" onclick={() => (showCreateForm = false)}>Cancel</button>
				<button type="button" onclick={submitCreateBranch}>Create</button>
			</div>
		</div>
	</div>
{/if}

<!-- Context menu -->
{#if contextMenu}
	<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
	<div class="ctx-backdrop" onclick={hideContextMenu} role="presentation"></div>
	<div class="ctx-menu" style="left: {contextMenu.x}px; top: {contextMenu.y}px;" role="menu">
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
		user-select: none;
	}

	/* ── Repo header ── */
	.repo-header {
		padding: 10px 12px 8px;
		border-bottom: 1px solid var(--border);
		flex-shrink: 0;
	}

	.repo-row {
		display: flex;
		align-items: center;
		gap: 6px;
		height: 20px;
	}

	.row-icon {
		color: var(--text-muted);
		flex-shrink: 0;
	}

	.repo-label {
		font-size: 10px;
		color: var(--text-muted);
		text-transform: lowercase;
		letter-spacing: 0.05em;
		flex-shrink: 0;
	}

	.repo-name {
		font-size: 12px;
		font-weight: 600;
		color: var(--text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.branch-current-name {
		font-size: 12px;
		color: var(--accent-green);
		font-weight: 500;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	/* ── Section list ── */
	.section-list {
		flex: 1;
		overflow-y: auto;
		min-height: 0;
	}

	.section {
		border-bottom: 1px solid var(--border);
	}

	.section-header-row {
		display: flex;
		align-items: center;
		gap: 6px;
		width: 100%;
		height: 30px;
		padding: 0 10px 0 8px;
		background: none;
		border: none;
		cursor: pointer;
		text-align: left;
	}
	.section-header-row:hover { background: rgba(255,255,255,0.03); }

	.full-btn {
		display: flex;
	}

	.section-toggle {
		flex: 1;
		display: flex;
		align-items: center;
		gap: 6px;
		background: none;
		border: none;
		cursor: pointer;
		color: inherit;
		font: inherit;
		padding: 0;
	}

	.chevron {
		font-size: 9px;
		color: var(--text-muted);
	}

	.section-label {
		font-size: 10px;
		font-weight: 600;
		color: var(--text-muted);
		letter-spacing: 0.08em;
		text-transform: uppercase;
		flex: 1;
		text-align: left;
	}

	.count-pill {
		font-size: 10px;
		padding: 1px 6px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 8px;
		color: var(--text-muted);
	}

	.section-add {
		width: 18px;
		height: 18px;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 14px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 3px;
		color: var(--text-muted);
		cursor: pointer;
		line-height: 1;
	}
	.section-add:hover { color: var(--accent-green); border-color: var(--accent-green); }

	/* ── Search ── */
	.search-wrap {
		padding: 4px 8px 6px;
	}

	.search-input {
		width: 100%;
		padding: 4px 8px;
		font-size: 11px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 4px;
		color: var(--text-primary);
	}
	.search-input::placeholder { color: var(--text-muted); }
	.search-input:focus { outline: none; border-color: var(--accent-blue); }

	/* ── Branch rows ── */
	.branch-list { padding: 2px 0 4px; }

	.branch-row {
		display: flex;
		align-items: center;
		gap: 6px;
		height: 26px;
		padding: 0 8px;
		font-size: 12px;
		cursor: pointer;
		transition: background 0.08s;
	}
	.branch-row:hover { background: var(--bg-tertiary); }

	.branch-row.is-head {
		background: rgba(63,185,80,0.08);
		color: var(--accent-green);
		font-weight: 600;
	}
	.branch-row.is-head:hover { background: rgba(63,185,80,0.14); }

	.branch-row.remote { color: var(--text-secondary); }

	.head-dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--accent-green);
		flex-shrink: 0;
		box-shadow: 0 0 4px var(--accent-green);
	}

	.branch-icon {
		color: var(--text-muted);
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
		font-size: 10px;
		color: var(--text-muted);
		font-family: 'JetBrains Mono', ui-monospace, monospace;
		flex-shrink: 0;
	}

	.checkout-btn {
		opacity: 0;
		width: 18px;
		height: 18px;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 13px;
		font-weight: bold;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 3px;
		color: var(--text-muted);
		cursor: pointer;
		flex-shrink: 0;
		transition: opacity 0.08s;
	}
	.branch-row:hover .checkout-btn { opacity: 1; }
	.checkout-btn:hover { color: var(--accent-green); border-color: var(--accent-green); }

	.remote-group { padding-left: 8px; }
	.remote-label {
		padding: 4px 8px;
		font-size: 10px;
		color: var(--text-muted);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.empty {
		padding: 6px 12px;
		font-size: 11px;
		color: var(--text-muted);
	}

	/* ── Bottom bar ── */
	.sidebar-bottom {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 4px;
		padding: 6px 8px;
		border-top: 1px solid var(--border);
		flex-shrink: 0;
	}

	.app-brand-name {
		font-size: 16px;
		font-weight: 800;
		color: var(--text-primary);
		letter-spacing: 0.5px;
		padding-left: 6px;
		padding-bottom: 2px;
	}

	.bottom-actions {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.bottom-icon-btn {
		width: 26px;
		height: 26px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: transparent;
		border: 1px solid transparent;
		border-radius: 4px;
		color: var(--text-muted);
		cursor: pointer;
		transition: color 0.1s, background 0.1s;
	}
	.bottom-icon-btn:hover { color: var(--text-primary); background: var(--bg-tertiary); border-color: var(--border); }

	/* ── Context menu ── */
	.ctx-backdrop {
		position: fixed;
		inset: 0;
		z-index: 999;
		background: transparent;
	}

	.ctx-menu {
		position: fixed;
		z-index: 1000;
		min-width: 180px;
		padding: 4px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 6px;
		box-shadow: 0 8px 24px rgba(0,0,0,0.5);
	}

	.ctx-menu button {
		display: block;
		width: 100%;
		padding: 7px 12px;
		background: none;
		border: none;
		color: var(--text-primary);
		font-size: 12px;
		text-align: left;
		cursor: pointer;
		border-radius: 4px;
	}
	.ctx-menu button:hover { background: var(--bg-secondary); }

	/* ── Modal ── */
	.modal-overlay {
		position: fixed;
		inset: 0;
		z-index: 1001;
		background: rgba(0,0,0,0.6);
		display: flex;
		align-items: center;
		justify-content: center;
		animation: fadeIn 0.15s ease;
	}
	@keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } }

	.modal {
		background: var(--bg-secondary);
		border: 1px solid var(--border);
		border-radius: 8px;
		padding: 24px;
		min-width: 320px;
		box-shadow: 0 16px 48px rgba(0,0,0,0.6);
	}

	.modal h3 { margin: 0 0 16px; font-size: 15px; }

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
	.modal input:focus { outline: none; border-color: var(--accent-blue); }

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
		color: #0d1117;
		border-color: var(--accent-green);
		font-weight: 600;
	}
</style>
