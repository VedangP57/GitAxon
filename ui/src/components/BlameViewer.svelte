<script lang="ts">
	import type { BlameLine } from '$lib/tauri';

	let {
		lines,
		onhashclick
	}: {
		lines: BlameLine[];
		onhashclick: (hash: string) => void;
	} = $props();

	function relativeDate(timestamp: number): string {
		const diff = Math.floor(Date.now() / 1000 - timestamp);
		if (diff < 60) return 'just now';
		if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
		if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
		if (diff < 2592000) return `${Math.floor(diff / 86400)}d ago`;
		return new Date(timestamp * 1000).toLocaleDateString();
	}
</script>

<div class="blame-viewer">
	{#each lines as line (line.line_no)}
		<div class="blame-row">
			<div class="blame-gutter">
				<button
					class="blame-hash"
					onclick={() => onhashclick(line.commit_hash)}
					title="{line.summary}\n{line.author} · {line.date}"
				>{line.short_hash}</button>
				<span class="blame-author">{line.author.split(' ')[0]}</span>
				<span class="blame-date">{relativeDate(line.timestamp)}</span>
			</div>
			<span class="blame-line-no">{line.line_no}</span>
			<span class="blame-content">{line.content}</span>
		</div>
	{/each}
</div>

<style>
	.blame-viewer {
		font-family: 'JetBrains Mono', 'Fira Code', ui-monospace, monospace;
		font-size: 12px;
		overflow: auto;
		height: 100%;
		background: var(--bg-primary);
	}
	.blame-row {
		display: flex;
		align-items: baseline;
		min-height: 20px;
		line-height: 20px;
	}
	.blame-row:hover {
		background: var(--bg-secondary);
	}
	.blame-gutter {
		display: flex;
		gap: 6px;
		padding: 0 8px;
		min-width: 220px;
		max-width: 220px;
		background: var(--bg-secondary);
		border-right: 1px solid var(--border);
		overflow: hidden;
		flex-shrink: 0;
	}
	.blame-hash {
		background: none;
		border: none;
		color: var(--accent-blue);
		font-family: inherit;
		font-size: 11px;
		cursor: pointer;
		padding: 0;
		flex-shrink: 0;
	}
	.blame-hash:hover {
		text-decoration: underline;
	}
	.blame-author {
		color: var(--text-secondary);
		font-size: 11px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		flex: 1;
	}
	.blame-date {
		color: var(--text-muted);
		font-size: 10px;
		flex-shrink: 0;
	}
	.blame-line-no {
		color: var(--text-muted);
		padding: 0 8px;
		min-width: 48px;
		text-align: right;
		user-select: none;
		flex-shrink: 0;
		border-right: 1px solid var(--bg-tertiary, var(--border));
	}
	.blame-content {
		color: var(--text-primary);
		white-space: pre;
		flex: 1;
		padding: 0 8px;
	}
</style>
