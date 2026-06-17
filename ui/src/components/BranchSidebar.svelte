<script lang="ts">
	import { branches, tags, prs, issues, repoCoords, currentRepo, loadRepo, openCreateBranchForm, createBranchFromHash, refreshGitHub, openPrReview } from '$lib/store';
	import {
		deleteBranch,
		renameBranch,
		mergeBranch,
		createTag,
		deleteTag,
		pushTag,
		setApiToken,
		getApiToken,
		addRemote as addRemoteIpc,
		removeRemote as removeRemoteIpc,
		renameRemote as renameRemoteIpc,
		setRemoteUrl as setRemoteUrlIpc,
		listWorktrees,
		addWorktree as addWorktreeIpc,
		removeWorktree as removeWorktreeIpc
	} from '$lib/tauri';
	import { invoke } from '@tauri-apps/api/core';
	import { get } from 'svelte/store';
	import { showToast } from '$lib/toast';
	import type { BranchInfo, TagInfo, PullRequest, GitIssue, WorktreeInfo } from '$lib/types';
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
	const tagList = $derived($tags);
	const prList = $derived($prs);
	const issueList = $derived($issues);
	const coords = $derived($repoCoords);

	// Remote management state
	let remoteCtxMenu = $state<{ x: number; y: number; remoteName: string } | null>(null);
	let showAddRemoteModal = $state(false);
	let newRemoteName = $state('');
	let newRemoteUrl = $state('');
	let editRemoteModal = $state<{ remoteName: string; url: string } | null>(null);
	let renameRemoteModal = $state<{ oldName: string; newName: string } | null>(null);

	async function handleAddRemote() {
		const repo = get(currentRepo);
		if (!repo || !newRemoteName.trim() || !newRemoteUrl.trim()) return;
		try {
			await addRemoteIpc(repo, newRemoteName.trim(), newRemoteUrl.trim());
			showToast(`Added remote '${newRemoteName.trim()}'`, 'success');
			newRemoteName = '';
			newRemoteUrl = '';
			showAddRemoteModal = false;
			await loadRepo(repo);
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	async function handleRemoveRemote(name: string) {
		const repo = get(currentRepo);
		if (!repo) return;
		if (!confirm(`Remove remote '${name}'? This cannot be undone.`)) return;
		try {
			await removeRemoteIpc(repo, name);
			showToast(`Removed remote '${name}'`, 'success');
			remoteCtxMenu = null;
			await loadRepo(repo);
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	async function handleRenameRemote() {
		const repo = get(currentRepo);
		if (!repo || !renameRemoteModal) return;
		const { oldName, newName } = renameRemoteModal;
		if (!newName.trim()) return;
		try {
			await renameRemoteIpc(repo, oldName, newName.trim());
			showToast(`Renamed remote '${oldName}' → '${newName.trim()}'`, 'success');
			renameRemoteModal = null;
			await loadRepo(repo);
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	async function handleSetRemoteUrl() {
		const repo = get(currentRepo);
		if (!repo || !editRemoteModal) return;
		const { remoteName, url } = editRemoteModal;
		if (!url.trim()) return;
		try {
			await setRemoteUrlIpc(repo, remoteName, url.trim());
			showToast(`Updated URL for '${remoteName}'`, 'success');
			editRemoteModal = null;
			await loadRepo(repo);
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	// Worktree state
	let worktreesOpen = $state(false);
	let worktreeList = $state<WorktreeInfo[]>([]);
	let showAddWorktreeModal = $state(false);
	let newWorktreePath = $state('');
	let newWorktreeBranch = $state('');

	async function loadWorktrees() {
		const repo = get(currentRepo);
		if (!repo) return;
		try {
			worktreeList = await listWorktrees(repo);
		} catch {
			worktreeList = [];
		}
	}

	async function handleAddWorktree() {
		const repo = get(currentRepo);
		if (!repo || !newWorktreePath.trim() || !newWorktreeBranch.trim()) return;
		try {
			const msg = await addWorktreeIpc(repo, newWorktreePath.trim(), newWorktreeBranch.trim());
			showToast(msg, 'success');
			newWorktreePath = '';
			newWorktreeBranch = '';
			showAddWorktreeModal = false;
			await loadWorktrees();
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	async function handleRemoveWorktree(wt: WorktreeInfo) {
		if (wt.is_main) { showToast('Cannot remove main worktree', 'error'); return; }
		const repo = get(currentRepo);
		if (!repo) return;
		if (!confirm(`Remove worktree at ${wt.path}?`)) return;
		try {
			const msg = await removeWorktreeIpc(repo, wt.path);
			showToast(msg, 'success');
			await loadWorktrees();
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	async function switchWorktree(wt: WorktreeInfo) {
		if (wt.path === get(currentRepo)) return;
		await loadRepo(wt.path);
		await loadWorktrees();
	}

	// Load worktrees when repo changes
	$effect(() => {
		if ($currentRepo) loadWorktrees();
	});

	let tagContextMenu = $state<{ x: number; y: number; tag: TagInfo } | null>(null);
	let showCreateTagForm = $state(false);
	let newTagName = $state('');
	let newTagMessage = $state('');
	let newTagTarget = $state('HEAD');

	async function handleCreateTag() {
		const repo = get(currentRepo);
		if (!repo || !newTagName.trim()) return;
		try {
			await createTag(repo, newTagName.trim(), newTagTarget.trim() || 'HEAD', newTagMessage.trim());
			showToast(`Created tag '${newTagName.trim()}'`, 'success');
			newTagName = '';
			newTagMessage = '';
			newTagTarget = 'HEAD';
			showCreateTagForm = false;
			await loadRepo(repo);
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	async function handleDeleteTag(tag: TagInfo) {
		tagContextMenu = null;
		const repo = get(currentRepo);
		if (!repo) return;
		try {
			await deleteTag(repo, tag.name);
			showToast(`Deleted tag '${tag.name}'`, 'success');
			await loadRepo(repo);
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	async function handlePushTag(tag: TagInfo) {
		tagContextMenu = null;
		const repo = get(currentRepo);
		if (!repo) return;
		try {
			const msg = await pushTag(repo, 'origin', tag.name);
			showToast(msg, 'success');
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	function formatTimeAgo(isoDate: string): string {
		const sec = Math.floor((Date.now() - new Date(isoDate).getTime()) / 1000);
		if (sec < 60) return 'just now';
		if (sec < 3600) return `${Math.floor(sec / 60)}m ago`;
		if (sec < 86400) return `${Math.floor(sec / 3600)}h ago`;
		return `${Math.floor(sec / 86400)}d ago`;
	}

	function openUrl(url: string) {
		window.open(url, '_blank');
	}

	// ─── Token Management ──────────────────────────────────────────────
	let showTokenModal = $state(false);
	let tokenInput = $state('');
	let tokenSaved = $state(false);

	async function openTokenModal() {
		const existing = await getApiToken('github').catch(() => '');
		tokenInput = existing;
		tokenSaved = !!existing;
		showTokenModal = true;
	}

	async function handleSaveToken() {
		if (!tokenInput.trim()) return;
		try {
			await setApiToken('github', tokenInput.trim());
			tokenSaved = true;
			showToast('GitHub token saved', 'success');
			showTokenModal = false;
			await refreshGitHub();
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	async function handleRemoveToken() {
		try {
			await setApiToken('github', '');
			tokenSaved = false;
			tokenInput = '';
			showToast('GitHub token removed', 'info');
			showTokenModal = false;
			await refreshGitHub();
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	function handleCopyTagHash(tag: TagInfo) {
		tagContextMenu = null;
		navigator.clipboard.writeText(tag.hash).catch(() => {});
		showToast('Copied tag hash', 'info');
	}
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
			<div class="section-header-row">
				<button class="full-btn" onclick={() => (remotesOpen = !remotesOpen)} style="flex:1; display:flex; align-items:center; gap:6px; background:none; border:none; color:inherit; cursor:pointer; padding:0;">
					<span class="chevron">{remotesOpen ? '▼' : '▶'}</span>
					<span class="section-label">REMOTE</span>
					<span class="count-pill">{filteredRemote.length}</span>
				</button>
				<button class="section-action-btn" title="Add remote" onclick={() => (showAddRemoteModal = true)}>+</button>
			</div>
			{#if remotesOpen}
				{#each Array.from(remoteGroups.entries()) as [remoteName, remoteBranches]}
					<div class="remote-group">
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<div class="remote-label" oncontextmenu={(e) => { e.preventDefault(); remoteCtxMenu = { x: e.clientX, y: e.clientY, remoteName }; }}>{remoteName}</div>
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

		<!-- ▼ WORKTREES -->
		<div class="section">
			<div class="section-header-row">
				<button class="full-btn" onclick={() => (worktreesOpen = !worktreesOpen)} style="flex:1; display:flex; align-items:center; gap:6px; background:none; border:none; color:inherit; cursor:pointer; padding:0;">
					<span class="chevron">{worktreesOpen ? '▼' : '▶'}</span>
					<span class="section-label">WORKTREES</span>
					<span class="count-pill">{worktreeList.length}</span>
				</button>
				<button class="section-action-btn" title="Add worktree" onclick={() => (showAddWorktreeModal = true)}>+</button>
			</div>
			{#if worktreesOpen}
				{#if worktreeList.length === 0}
					<div class="empty">No worktrees</div>
				{:else}
					{#each worktreeList as wt (wt.path)}
						<div
							class="branch-row"
							class:active={wt.path === $currentRepo}
							onclick={() => switchWorktree(wt)}
							oncontextmenu={(e) => { e.preventDefault(); if (!wt.is_main) handleRemoveWorktree(wt); }}
							role="button"
							tabindex="0"
							onkeydown={(e) => (e.key === 'Enter') && switchWorktree(wt)}
							title={wt.path}
						>
							<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="branch-icon"><path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>
							<span class="branch-name">{wt.branch ?? 'detached'}</span>
							<span class="branch-hash">{wt.head_hash.slice(0, 7)}</span>
							{#if wt.is_main}
								<span class="head-badge">main</span>
							{/if}
						</div>
					{/each}
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
				<span class="count-pill">{prList.length}</span>
			</button>
			{#if prOpen}
				<div class="section-actions">
					<button class="section-action-btn" title="Configure GitHub token" onclick={openTokenModal}>⚙</button>
					<button class="section-action-btn" title="Refresh" onclick={refreshGitHub}>↻</button>
				</div>
				{#if !coords}
					<div class="empty">Not a GitHub/GitLab repo</div>
				{:else if prList.length === 0}
					<div class="empty">No open PRs</div>
				{:else}
					{#each prList as pr (pr.number)}
						<div class="gh-item" role="button" tabindex="0" onclick={() => openPrReview(pr.number)} onkeydown={(e) => { if (e.key === 'Enter') openPrReview(pr.number); }} title="#{pr.number} — {pr.headBranch} → {pr.baseBranch}">
							<div class="gh-item-header">
								<span class="gh-number">#{pr.number}</span>
								{#if pr.draft}
									<span class="gh-badge gh-draft">Draft</span>
								{:else}
									<span class="gh-badge gh-open">Open</span>
								{/if}
								<span class="gh-time">{formatTimeAgo(pr.updatedAt)}</span>
							</div>
							<div class="gh-title">{pr.title}</div>
							<div class="gh-meta">{pr.author} · {pr.headBranch} → {pr.baseBranch}</div>
						</div>
					{/each}
				{/if}
			{/if}
		</div>

		<!-- ▼ ISSUES -->
		<div class="section">
			<button class="section-header-row full-btn" onclick={() => (issuesOpen = !issuesOpen)}>
				<span class="chevron">{issuesOpen ? '▼' : '▶'}</span>
				<span class="section-label">ISSUES</span>
				<span class="count-pill">{issueList.length}</span>
			</button>
			{#if issuesOpen}
				{#if !coords}
					<div class="empty">Not a GitHub/GitLab repo</div>
				{:else if issueList.length === 0}
					<div class="empty">No open issues</div>
				{:else}
					{#each issueList as issue (issue.number)}
						<div class="gh-item" role="button" tabindex="0" onclick={() => openUrl(issue.url)} onkeydown={(e) => { if (e.key === 'Enter') openUrl(issue.url); }} title="#{issue.number}">
							<div class="gh-item-header">
								<span class="gh-number">#{issue.number}</span>
								<span class="gh-badge gh-open">Open</span>
								<span class="gh-time">{formatTimeAgo(issue.updatedAt)}</span>
							</div>
							<div class="gh-title">{issue.title}</div>
							<div class="gh-meta">
								{issue.author}
								{#if issue.labels.length > 0}
									· {issue.labels.slice(0, 3).join(', ')}
								{/if}
							</div>
						</div>
					{/each}
				{/if}
			{/if}
		</div>

		<!-- ▼ TAGS -->
		<div class="section">
			<button class="section-header-row full-btn" onclick={() => (tagsOpen = !tagsOpen)}>
				<span class="chevron">{tagsOpen ? '▼' : '▶'}</span>
				<span class="section-label">TAGS</span>
				<span class="count-pill">{tagList.length}</span>
			</button>
			{#if tagsOpen}
				<div class="section-actions">
					<button class="section-action-btn" title="Create tag" onclick={() => (showCreateTagForm = true)}>+</button>
				</div>
				{#if tagList.length === 0}
					<div class="empty">No tags</div>
				{:else}
					{#each tagList as tag (tag.name)}
						<div
							class="branch-row tag-row"
							role="none"
							oncontextmenu={(e) => {
								e.preventDefault();
								tagContextMenu = { x: e.clientX, y: e.clientY, tag };
							}}
							title={tag.isAnnotated ? `${tag.name}: ${tag.message}` : tag.name}
						>
							<span class="tag-icon">🏷</span>
							<span class="branch-name">{tag.name}</span>
							{#if tag.isAnnotated}
								<span class="tag-annotated-badge">A</span>
							{/if}
						</div>
					{/each}
				{/if}
			{/if}
		</div>

		{#if showCreateTagForm}
			<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
			<div class="modal-overlay" onclick={() => (showCreateTagForm = false)} role="presentation">
				<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
				<div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-label="Create tag" tabindex="-1">
					<div class="modal-title">Create Tag</div>
					<input class="modal-input" type="text" placeholder="Tag name" bind:value={newTagName} />
					<input class="modal-input" type="text" placeholder="Target (commit hash or HEAD)" bind:value={newTagTarget} />
					<input class="modal-input" type="text" placeholder="Message (optional, for annotated tag)" bind:value={newTagMessage} />
					<div class="modal-actions">
						<button class="modal-btn cancel" onclick={() => (showCreateTagForm = false)}>Cancel</button>
						<button class="modal-btn confirm" onclick={handleCreateTag} disabled={!newTagName.trim()}>Create</button>
					</div>
				</div>
			</div>
		{/if}

		{#if tagContextMenu}
			<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
			<div class="ctx-backdrop" onclick={() => (tagContextMenu = null)} role="presentation"></div>
			<div class="ctx-menu" style="top: {tagContextMenu.y}px; left: {tagContextMenu.x}px">
				<div class="ctx-header">{tagContextMenu.tag.name}</div>
				<button class="ctx-item" onclick={() => handlePushTag(tagContextMenu!.tag)}>Push to origin</button>
				<button class="ctx-item" onclick={() => handleCopyTagHash(tagContextMenu!.tag)}>Copy hash</button>
				<div class="ctx-divider"></div>
				<button class="ctx-item ctx-danger" onclick={() => handleDeleteTag(tagContextMenu!.tag)}>Delete tag</button>
			</div>
		{/if}

		{#if remoteCtxMenu}
			<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
			<div class="ctx-backdrop" onclick={() => (remoteCtxMenu = null)} role="presentation"></div>
			<div class="ctx-menu" style="top: {remoteCtxMenu.y}px; left: {remoteCtxMenu.x}px">
				<div class="ctx-header">{remoteCtxMenu.remoteName}</div>
				<button class="ctx-item" onclick={() => { renameRemoteModal = { oldName: remoteCtxMenu!.remoteName, newName: '' }; remoteCtxMenu = null; }}>Rename remote</button>
				<button class="ctx-item" onclick={() => { editRemoteModal = { remoteName: remoteCtxMenu!.remoteName, url: '' }; remoteCtxMenu = null; }}>Edit URL</button>
				<div class="ctx-divider"></div>
				<button class="ctx-item ctx-danger" onclick={() => handleRemoveRemote(remoteCtxMenu!.remoteName)}>Remove remote</button>
			</div>
		{/if}

		{#if showAddRemoteModal}
			<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
			<div class="modal-overlay" onclick={() => (showAddRemoteModal = false)} role="presentation">
				<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
				<div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-label="Add remote" tabindex="-1">
					<h3>Add Remote</h3>
					<label>Name<input class="modal-input" type="text" placeholder="e.g. upstream" bind:value={newRemoteName} /></label>
					<label>URL<input class="modal-input" type="text" placeholder="https://github.com/..." bind:value={newRemoteUrl} onkeydown={(e) => e.key === 'Enter' && handleAddRemote()} /></label>
					<div class="modal-actions">
						<button class="modal-btn cancel" onclick={() => (showAddRemoteModal = false)}>Cancel</button>
						<button class="modal-btn confirm" onclick={handleAddRemote} disabled={!newRemoteName.trim() || !newRemoteUrl.trim()}>Add</button>
					</div>
				</div>
			</div>
		{/if}

		{#if renameRemoteModal}
			<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
			<div class="modal-overlay" onclick={() => (renameRemoteModal = null)} role="presentation">
				<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
				<div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-label="Rename remote" tabindex="-1">
					<h3>Rename Remote: {renameRemoteModal.oldName}</h3>
					<label>New name<input class="modal-input" type="text" placeholder="New remote name" bind:value={renameRemoteModal.newName} onkeydown={(e) => e.key === 'Enter' && handleRenameRemote()} /></label>
					<div class="modal-actions">
						<button class="modal-btn cancel" onclick={() => (renameRemoteModal = null)}>Cancel</button>
						<button class="modal-btn confirm" onclick={handleRenameRemote} disabled={!renameRemoteModal.newName.trim()}>Rename</button>
					</div>
				</div>
			</div>
		{/if}

		{#if editRemoteModal}
			<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
			<div class="modal-overlay" onclick={() => (editRemoteModal = null)} role="presentation">
				<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
				<div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-label="Edit remote URL" tabindex="-1">
					<h3>Edit URL: {editRemoteModal.remoteName}</h3>
					<label>URL<input class="modal-input" type="text" placeholder="https://..." bind:value={editRemoteModal.url} onkeydown={(e) => e.key === 'Enter' && handleSetRemoteUrl()} /></label>
					<div class="modal-actions">
						<button class="modal-btn cancel" onclick={() => (editRemoteModal = null)}>Cancel</button>
						<button class="modal-btn confirm" onclick={handleSetRemoteUrl} disabled={!editRemoteModal.url.trim()}>Save</button>
					</div>
				</div>
			</div>
		{/if}

		{#if showAddWorktreeModal}
			<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
			<div class="modal-overlay" onclick={() => (showAddWorktreeModal = false)} role="presentation">
				<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
				<div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-label="Add worktree" tabindex="-1">
					<h3>Add Worktree</h3>
					<label>Path<input class="modal-input" type="text" placeholder="/path/to/worktree" bind:value={newWorktreePath} /></label>
					<label>Branch<input class="modal-input" type="text" placeholder="branch-name" bind:value={newWorktreeBranch} onkeydown={(e) => e.key === 'Enter' && handleAddWorktree()} /></label>
					<div class="modal-actions">
						<button class="modal-btn cancel" onclick={() => (showAddWorktreeModal = false)}>Cancel</button>
						<button class="modal-btn confirm" onclick={handleAddWorktree} disabled={!newWorktreePath.trim() || !newWorktreeBranch.trim()}>Create</button>
					</div>
				</div>
			</div>
		{/if}

		{#if showTokenModal}
			<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
			<div class="modal-overlay" onclick={() => (showTokenModal = false)} role="presentation">
				<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
				<div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-label="GitHub Token" tabindex="-1">
					<div class="modal-title">GitHub Personal Access Token</div>
					<p class="token-hint">
						Create a token at GitHub → Settings → Developer settings → Personal access tokens.
						Required scopes: <code>repo</code>, <code>read:org</code>
					</p>
					<input
						class="modal-input"
						type="password"
						placeholder="ghp_xxxxxxxxxxxxxxxxxxxx"
						bind:value={tokenInput}
					/>
					<div class="modal-actions">
						{#if tokenSaved}
							<button class="modal-btn cancel" onclick={handleRemoveToken}>Remove Token</button>
						{/if}
						<button class="modal-btn cancel" onclick={() => (showTokenModal = false)}>Cancel</button>
						<button class="modal-btn confirm" onclick={handleSaveToken} disabled={!tokenInput.trim()}>Save</button>
					</div>
				</div>
			</div>
		{/if}

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

	/* ── Tags ── */
	.tag-row {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.tag-icon {
		font-size: 11px;
		flex-shrink: 0;
	}

	.tag-annotated-badge {
		font-size: 9px;
		font-weight: 600;
		padding: 0 4px;
		border-radius: 3px;
		background: color-mix(in srgb, var(--accent-yellow) 18%, transparent);
		color: var(--accent-yellow);
		flex-shrink: 0;
	}

	.section-actions {
		display: flex;
		justify-content: flex-end;
		padding: 2px 8px;
	}

	.section-action-btn {
		background: none;
		border: 1px solid var(--border);
		color: var(--text-secondary);
		border-radius: 4px;
		width: 22px;
		height: 22px;
		font-size: 14px;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.section-action-btn:hover {
		background: var(--bg-tertiary);
		color: var(--text-primary);
	}

	.modal-title {
		margin: 0 0 16px;
		font-size: 15px;
		font-weight: 600;
		color: var(--text-primary);
	}

	.modal-input {
		display: block;
		width: 100%;
		margin-bottom: 8px;
		padding: 8px 12px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text-primary);
		font-size: 13px;
		box-sizing: border-box;
	}
	.modal-input:focus {
		outline: none;
		border-color: var(--accent-blue);
	}

	.modal-btn {
		padding: 8px 16px;
		border-radius: 6px;
		font-size: 13px;
		cursor: pointer;
		border: 1px solid var(--border);
	}
	.modal-btn.cancel {
		background: var(--bg-tertiary);
		color: var(--text-primary);
	}
	.modal-btn.confirm {
		background: var(--accent-green);
		color: #0d1117;
		border-color: var(--accent-green);
		font-weight: 600;
	}
	.modal-btn.confirm:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.ctx-header {
		padding: 6px 12px;
		font-size: 11px;
		color: var(--text-muted);
		font-weight: 600;
	}

	.ctx-item {
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
	.ctx-item:hover {
		background: var(--bg-secondary);
	}

	.ctx-danger {
		color: var(--accent-red) !important;
	}

	.ctx-divider {
		height: 1px;
		background: var(--border);
		margin: 4px 0;
	}

	.token-hint {
		font-size: 11px;
		color: var(--text-muted);
		margin: 0 0 12px;
		line-height: 1.5;
	}
	.token-hint code {
		background: var(--bg-tertiary);
		padding: 1px 4px;
		border-radius: 3px;
		font-size: 10px;
	}

	/* ── GitHub/GitLab items ── */
	.gh-item {
		padding: 6px 12px;
		cursor: pointer;
		border-radius: 4px;
	}
	.gh-item:hover {
		background: var(--bg-tertiary);
	}

	.gh-item-header {
		display: flex;
		align-items: center;
		gap: 6px;
		margin-bottom: 2px;
	}

	.gh-number {
		font-size: 11px;
		font-weight: 600;
		color: var(--accent-blue);
		font-family: 'JetBrains Mono', monospace;
	}

	.gh-badge {
		font-size: 9px;
		font-weight: 600;
		padding: 1px 5px;
		border-radius: 3px;
	}

	.gh-open {
		background: color-mix(in srgb, var(--accent-green) 18%, transparent);
		color: var(--accent-green);
	}

	.gh-draft {
		background: color-mix(in srgb, var(--text-muted) 18%, transparent);
		color: var(--text-muted);
	}

	.gh-time {
		font-size: 10px;
		color: var(--text-muted);
		margin-left: auto;
	}

	.gh-title {
		font-size: 12px;
		color: var(--text-primary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.gh-meta {
		font-size: 10px;
		color: var(--text-muted);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
