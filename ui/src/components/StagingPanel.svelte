<script lang="ts">
	import { onMount } from 'svelte';
	import {
		currentRepo,
		status,
		refreshStatus,
		loadRepo,
		selectFileFromStaging
	} from '$lib/store';
	import {
		stageFile,
		unstageFile,
		stageAll,
		unstageAll,
		createCommit,
		getGitConfig
	} from '$lib/tauri';
	import { showToast } from '$lib/toast';
	import type { IndexEntry } from '$lib/types';

	const unstagedStatuses = ['Unstaged', 'Untracked', 'Conflicted'] as const;
	const stagedStatus = 'Staged' as const;

	const unstagedFiles = $derived(
		$status.filter((e) =>
			unstagedStatuses.includes(e.status as (typeof unstagedStatuses)[number])
		)
	);
	const stagedFiles = $derived(
		$status.filter((e) => e.status === stagedStatus)
	);

	let authorName = $state('');
	let authorEmail = $state('');
	let commitMessage = $state('');
	let amend = $state(false);
	let isCommitting = $state(false);
	let isStaging = $state<string | null>(null);

	$effect(() => {
		const repo = $currentRepo;
		if (repo) {
			Promise.all([
				getGitConfig(repo, 'user.name').catch(() => ''),
				getGitConfig(repo, 'user.email').catch(() => '')
			]).then(([name, email]) => {
				authorName = name;
				authorEmail = email;
			});
		}
	});

	onMount(() => {
		const handler = () => handleCommit();
		window.addEventListener('gitfast:commit', handler);
		return () => window.removeEventListener('gitfast:commit', handler);
	});

	function getFileIcon(entry: IndexEntry): { char: string; class: string } {
		switch (entry.status) {
			case 'Untracked':
				return { char: '+', class: 'icon-untracked' };
			case 'Conflicted':
				return { char: '!', class: 'icon-conflicted' };
			case 'Staged':
			case 'Unstaged':
			default:
				return { char: '•', class: 'icon-modified' };
		}
	}

	function truncatePath(path: string, maxLen = 50): string {
		if (path.length <= maxLen) return path;
		return '...' + path.slice(-maxLen + 3);
	}

	async function handleStage(entry: IndexEntry) {
		const repo = $currentRepo;
		if (!repo) return;
		isStaging = entry.path;
		try {
			await stageFile(repo, entry.path);
			await refreshStatus();
		} finally {
			isStaging = null;
		}
	}

	async function handleStageAll() {
		const repo = $currentRepo;
		if (!repo) return;
		isStaging = '__all__';
		try {
			await stageAll(repo);
			await refreshStatus();
		} finally {
			isStaging = null;
		}
	}

	async function handleUnstage(entry: IndexEntry) {
		const repo = $currentRepo;
		if (!repo) return;
		isStaging = entry.path;
		try {
			await unstageFile(repo, entry.path);
			await refreshStatus();
		} finally {
			isStaging = null;
		}
	}

	async function handleUnstageAll() {
		const repo = $currentRepo;
		if (!repo) return;
		isStaging = '__all__';
		try {
			await unstageAll(repo);
			await refreshStatus();
		} finally {
			isStaging = null;
		}
	}

	async function handleRowClick(entry: IndexEntry, useStaged: boolean) {
		const repo = $currentRepo;
		if (!repo) return;
		await selectFileFromStaging(repo, entry.path, useStaged);
	}

	async function handleCommit() {
		const repo = $currentRepo;
		if (!repo || !commitMessage.trim() || stagedFiles.length === 0) return;
		isCommitting = true;
		try {
			await createCommit(repo, commitMessage.trim(), authorName, authorEmail);
			await loadRepo(repo);
			commitMessage = '';
			showToast('Commit created successfully', 'success');
		} catch (e) {
			showToast(e instanceof Error ? e.message : String(e), 'error');
		} finally {
			isCommitting = false;
		}
	}

	const canCommit = $derived(
		commitMessage.trim().length > 0 && stagedFiles.length > 0
	);
	const subjectLength = $derived(
		commitMessage.split('\n')[0]?.length ?? 0
	);
</script>

<div class="staging-panel">
	<div class="columns">
		<!-- Left: Unstaged Changes -->
		<div class="column">
			<div class="column-header">
				<span class="column-title">Unstaged Changes</span>
				<span class="badge">{unstagedFiles.length}</span>
				<button
					class="action-btn"
					onclick={handleStageAll}
					disabled={unstagedFiles.length === 0 || isStaging !== null}
					title="Stage All"
				>
					Stage All
				</button>
			</div>
			<div class="file-list">
				{#if unstagedFiles.length === 0}
					<div class="empty-state empty-unstaged">Working tree clean ✓</div>
				{:else}
				{#each unstagedFiles as entry (entry.path)}
					<div
						class="file-row"
						role="button"
						tabindex="0"
						onclick={() => handleRowClick(entry, false)}
						onkeydown={(e) =>
							e.key === 'Enter' && handleRowClick(entry, false)}
					>
						<span class="file-icon {getFileIcon(entry).class}">
							{getFileIcon(entry).char}
						</span>
						<span class="file-path" title={entry.path}>
							{truncatePath(entry.path)}
						</span>
						<button
							class="row-action row-action-stage"
							onclick={(e) => {
								e.stopPropagation();
								handleStage(entry);
							}}
							disabled={isStaging !== null}
							title="Stage"
						>
							+
						</button>
					</div>
				{/each}
				{/if}
			</div>
		</div>

		<div class="divider"></div>

		<!-- Right: Staged Changes -->
		<div class="column">
			<div class="column-header">
				<span class="column-title">Staged Changes</span>
				<span class="badge">{stagedFiles.length}</span>
				<button
					class="action-btn"
					onclick={handleUnstageAll}
					disabled={stagedFiles.length === 0 || isStaging !== null}
					title="Unstage All"
				>
					Unstage All
				</button>
			</div>
			<div class="file-list">
				{#if stagedFiles.length === 0}
					<div class="empty-state empty-staged">Nothing staged</div>
				{:else}
				{#each stagedFiles as entry (entry.path)}
					<div
						class="file-row"
						role="button"
						tabindex="0"
						onclick={() => handleRowClick(entry, true)}
						onkeydown={(e) =>
							e.key === 'Enter' && handleRowClick(entry, true)}
					>
						<span class="file-icon {getFileIcon(entry).class}">
							{getFileIcon(entry).char}
						</span>
						<span class="file-path" title={entry.path}>
							{truncatePath(entry.path)}
						</span>
						<button
							class="row-action row-action-unstage"
							onclick={(e) => {
								e.stopPropagation();
								handleUnstage(entry);
							}}
							disabled={isStaging !== null}
							title="Unstage"
						>
							−
						</button>
					</div>
				{/each}
				{/if}
			</div>
		</div>
	</div>

	<!-- Bottom: Commit Box -->
	<div class="commit-box">
		<div class="author-info">
			Committing as: {authorName || '(not set)'}
		</div>
		<div class="commit-inputs">
			<textarea
				class="commit-textarea"
				placeholder="Commit message (required)"
				bind:value={commitMessage}
				rows="3"
			></textarea>
			<div class="commit-actions">
				<span class="char-count" class:visible={subjectLength > 0}>
					{subjectLength} chars
				</span>
				<button
					class="amend-btn"
					class:active={amend}
					onclick={() => (amend = !amend)}
					title="Amend last commit"
				>
					Amend
				</button>
				<button
					class="commit-btn"
					disabled={!canCommit || isCommitting}
					onclick={handleCommit}
				>
					{isCommitting ? 'Committing...' : 'Commit'}
				</button>
			</div>
		</div>
	</div>
</div>

<style>
	.staging-panel {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
		background: var(--bg-secondary);
		border-radius: 8px;
		border: 1px solid var(--border);
		overflow: hidden;
	}

	.columns {
		display: flex;
		flex: 1;
		min-height: 0;
	}

	.column {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-width: 0;
		overflow: hidden;
	}

	.divider {
		width: 1px;
		background: var(--border);
		flex-shrink: 0;
	}

	.column-header {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 12px;
		background: var(--bg-tertiary);
		border-bottom: 1px solid var(--border);
		flex-shrink: 0;
	}

	.column-title {
		font-size: 13px;
		font-weight: 600;
		color: var(--text-primary);
	}

	.badge {
		font-size: 12px;
		padding: 2px 8px;
		background: var(--border);
		border-radius: 10px;
		color: var(--text-secondary);
	}

	.action-btn {
		margin-left: auto;
		font-size: 12px;
		padding: 4px 10px;
		background: var(--bg-primary);
		border: 1px solid var(--border);
		color: var(--text-primary);
		border-radius: 4px;
		cursor: pointer;
	}
	.action-btn:hover:not(:disabled) {
		background: var(--border);
	}
	.action-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.file-list {
		flex: 1;
		overflow-y: auto;
		padding: 4px 0;
	}

	.file-row {
		display: flex;
		align-items: center;
		height: 28px;
		padding: 0 12px;
		font-size: 13px;
		cursor: pointer;
		gap: 8px;
		position: relative;
	}
	.file-row:hover {
		background: var(--bg-tertiary);
	}
	.file-row .row-action {
		opacity: 0;
		margin-left: auto;
		font-size: 11px;
		padding: 2px 8px;
		background: var(--bg-primary);
		border: 1px solid var(--border);
		color: var(--text-secondary);
		border-radius: 4px;
		cursor: pointer;
		position: absolute;
		right: 12px;
	}
	.file-row:hover .row-action {
		opacity: 1;
	}
	.file-row .row-action-stage:hover {
		background: var(--accent-green);
		color: white;
		border-color: var(--accent-green);
	}
	.file-row .row-action-stage {
		font-size: 14px;
		font-weight: bold;
		line-height: 1;
		padding: 2px 6px;
	}
	.file-row .row-action:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
	.file-row .row-action-unstage:hover {
		background: var(--accent-orange);
		color: white;
		border-color: var(--accent-orange);
	}
	.file-row .row-action-unstage {
		font-size: 14px;
		font-weight: bold;
		line-height: 1;
		padding: 2px 6px;
	}

	.file-icon {
		flex-shrink: 0;
		width: 16px;
		text-align: center;
		font-size: 12px;
		font-weight: bold;
	}
	.icon-modified {
		color: var(--accent-orange);
	}
	.icon-untracked {
		color: var(--accent-green);
	}
	.icon-conflicted {
		color: var(--accent-orange);
	}

	.file-path {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--text-primary);
	}

	.empty-state {
		padding: 16px 12px;
		font-size: 12px;
		color: var(--text-muted);
		text-align: center;
	}
	.empty-unstaged {
		color: var(--accent-green);
	}
	.empty-staged {
		color: var(--text-muted);
	}

	.commit-box {
		padding: 12px;
		background: var(--bg-tertiary);
		border-top: 1px solid var(--border);
		flex-shrink: 0;
	}

	.author-info {
		font-size: 11px;
		color: var(--text-muted);
		margin-bottom: 6px;
	}

	.commit-inputs {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.commit-textarea {
		min-height: 60px;
		padding: 10px 12px;
		background: var(--bg-primary);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text-primary);
		font-size: 13px;
		font-family: inherit;
		resize: vertical;
	}
	.commit-textarea::placeholder {
		color: var(--text-muted);
	}
	.commit-textarea:focus {
		outline: none;
		border-color: var(--accent-blue);
	}

	.commit-actions {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.char-count {
		font-size: 11px;
		color: var(--text-muted);
		opacity: 0;
	}
	.char-count.visible {
		opacity: 1;
	}

	.amend-btn {
		font-size: 12px;
		padding: 6px 12px;
		background: var(--bg-primary);
		border: 1px solid var(--border);
		color: var(--text-secondary);
		border-radius: 4px;
		cursor: pointer;
	}
	.amend-btn:hover {
		background: var(--border);
		color: var(--text-primary);
	}
	.amend-btn.active {
		background: var(--border);
		color: var(--accent-blue);
		border-color: var(--accent-blue);
	}

	.commit-btn {
		margin-left: auto;
		font-size: 13px;
		padding: 8px 20px;
		background: var(--accent-green);
		border: 1px solid var(--accent-green);
		color: white;
		border-radius: 6px;
		cursor: pointer;
		font-weight: 500;
	}
	.commit-btn:hover:not(:disabled) {
		filter: brightness(1.1);
	}
	.commit-btn:disabled {
		background: var(--border);
		border-color: var(--border);
		color: var(--text-muted);
		cursor: not-allowed;
	}
</style>
