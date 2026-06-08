<script lang="ts">
	import { fileHistory, fileHistoryPath, fileHistoryLoading, closeDiff, selectFileHistoryEntry } from '$lib/store';
	import type { FileHistoryEntry } from '$lib/types';

	let entries = $state<FileHistoryEntry[]>([]);
	let filePath = $state<string | null>(null);
	let loading = $state(false);

	$effect(() => {
		const unsub1 = fileHistory.subscribe((v) => (entries = v));
		const unsub2 = fileHistoryPath.subscribe((v) => (filePath = v));
		const unsub3 = fileHistoryLoading.subscribe((v) => (loading = v));
		return () => { unsub1(); unsub2(); unsub3(); };
	});

	function formatDate(ts: number): string {
		const d = new Date(ts * 1000);
		const now = new Date();
		const diffDays = Math.floor((now.getTime() - d.getTime()) / 86400000);
		if (diffDays === 0) return 'Today';
		if (diffDays === 1) return 'Yesterday';
		if (diffDays < 7) return `${diffDays} days ago`;
		return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
	}

	function handleClick(entry: FileHistoryEntry) {
		if (filePath) {
			selectFileHistoryEntry(filePath, entry.hash);
		}
	}
</script>

<div class="file-history-panel">
	<div class="fh-header">
		<button class="fh-back" onclick={() => closeDiff()} title="Back to graph">←</button>
		<div class="fh-title">
			<span class="fh-label">FILE HISTORY</span>
			{#if filePath}
				<span class="fh-path">{filePath}</span>
			{/if}
		</div>
		<span class="fh-count">{entries.length} commits</span>
	</div>

	<div class="fh-list">
		{#if loading}
			<div class="fh-loading">Loading history...</div>
		{:else if entries.length === 0}
			<div class="fh-empty">No history found</div>
		{:else}
			{#each entries as entry (entry.hash)}
				<button class="fh-entry" onclick={() => handleClick(entry)}>
					<div class="fh-entry-top">
						<span class="fh-hash">{entry.short_hash}</span>
						<span class="fh-date">{formatDate(entry.timestamp)}</span>
					</div>
					<div class="fh-message">{entry.message}</div>
					<div class="fh-entry-bottom">
						<span class="fh-author">{entry.author_name}</span>
						<div class="fh-stats">
							{#if entry.additions > 0}
								<span class="fh-add">+{entry.additions}</span>
							{/if}
							{#if entry.deletions > 0}
								<span class="fh-del">-{entry.deletions}</span>
							{/if}
						</div>
					</div>
				</button>
			{/each}
		{/if}
	</div>
</div>

<style>
	.file-history-panel {
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--bg-primary);
		color: var(--text-primary);
	}

	.fh-header {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 12px;
		border-bottom: 1px solid var(--border);
		flex-shrink: 0;
	}

	.fh-back {
		background: none;
		border: none;
		color: var(--text-secondary);
		cursor: pointer;
		font-size: 16px;
		padding: 4px 8px;
		border-radius: 4px;
	}

	.fh-back:hover {
		background: var(--bg-secondary);
		color: var(--text-primary);
	}

	.fh-title {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.fh-label {
		font-size: 10px;
		font-weight: 600;
		letter-spacing: 0.5px;
		color: var(--text-muted);
		text-transform: uppercase;
	}

	.fh-path {
		font-size: 12px;
		color: var(--accent-blue);
		font-family: monospace;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.fh-count {
		font-size: 11px;
		color: var(--text-muted);
		flex-shrink: 0;
	}

	.fh-list {
		flex: 1;
		overflow-y: auto;
		padding: 4px 0;
	}

	.fh-loading, .fh-empty {
		padding: 24px;
		text-align: center;
		color: var(--text-muted);
		font-size: 13px;
	}

	.fh-entry {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: 8px 16px;
		border: none;
		background: transparent;
		cursor: pointer;
		text-align: left;
		width: 100%;
		border-bottom: 1px solid var(--border);
	}

	.fh-entry:hover {
		background: var(--bg-secondary);
	}

	.fh-entry-top {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.fh-hash {
		font-family: monospace;
		font-size: 11px;
		color: var(--accent-orange);
		font-weight: 600;
	}

	.fh-date {
		font-size: 10px;
		color: var(--text-muted);
	}

	.fh-message {
		font-size: 12px;
		color: var(--text-primary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.fh-entry-bottom {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.fh-author {
		font-size: 11px;
		color: var(--text-secondary);
	}

	.fh-stats {
		display: flex;
		gap: 8px;
		font-size: 11px;
		font-family: monospace;
	}

	.fh-add {
		color: var(--accent-green);
	}

	.fh-del {
		color: var(--accent-red);
	}
</style>
