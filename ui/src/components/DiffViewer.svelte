<script lang="ts">
	import {
		diffFile,
		diffMode,
		isDiffLoading,
		closeDiff,
		currentRepo
	} from '$lib/store';
	import { discardFile } from '$lib/tauri';
	import { showToast } from '$lib/toast';
	import type { DiffFile, DiffHunk } from '$lib/types';
	import { get } from 'svelte/store';

	let activeTab = $state<'diff' | 'file'>('diff');
	let showDiscardConfirm = $state(false);

	function getFilePath(file: DiffFile): string {
		return file.new_path ?? file.old_path ?? '';
	}

	function hunkHeader(hunk: DiffHunk): string {
		return `@@ -${hunk.old_start},${hunk.old_lines} +${hunk.new_start},${hunk.new_lines} @@`;
	}

	async function handleDiscardCurrentFile() {
		const repo = get(currentRepo);
		const file = get(diffFile);
		if (!repo || !file) return;
		const path = getFilePath(file);
		try {
			await discardFile(repo, path);
			showToast(`Discarded changes to ${path.split('/').pop()}`, 'success');
			closeDiff();
		} catch (e) {
			showToast(String(e), 'error');
		} finally {
			showDiscardConfirm = false;
		}
	}
</script>

<div class="diff-viewer">
	<!-- Header bar -->
	<div class="dv-header">
		<button class="back-btn" onclick={closeDiff} title="Back to graph">
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
				<path d="M19 12H5"/><polyline points="12 19 5 12 12 5"/>
			</svg>
			Graph
		</button>

		<div class="dv-filepath">
			{#if $diffFile}
				<span class="dv-mode-badge" class:mode-staged={$diffMode === 'staged'} class:mode-wt={$diffMode === 'working-tree'} class:mode-commit={$diffMode === 'commit'}>
					{$diffMode === 'staged' ? 'Staged' : $diffMode === 'working-tree' ? 'Working Tree' : 'Commit'}
				</span>
				{getFilePath($diffFile)}
			{/if}
		</div>

		<div class="dv-controls">
			<div class="toggle-group">
				<button class="toggle-btn" class:active={activeTab === 'file'} onclick={() => (activeTab = 'file')}>File View</button>
				<button class="toggle-btn" class:active={activeTab === 'diff'} onclick={() => (activeTab = 'diff')}>Diff View</button>
			</div>
			<div class="toggle-group">
				<button class="toggle-btn">Blame</button>
				<button class="toggle-btn">History</button>
			</div>
			{#if $diffMode === 'working-tree' && $diffFile}
				<button
					class="discard-file-btn"
					title="Discard all changes to this file"
					onclick={() => (showDiscardConfirm = !showDiscardConfirm)}
				>↺ Discard</button>
			{/if}
			<button class="close-btn" onclick={closeDiff} title="Close diff">✕</button>
		</div>
	</div>

	{#if showDiscardConfirm && $diffMode === 'working-tree'}
		<div class="discard-confirm-bar">
			<span class="dc-warn">⚠ Discard all changes to this file? This cannot be undone.</span>
			<button class="dc-btn-danger" onclick={handleDiscardCurrentFile}>Discard</button>
			<button class="dc-btn-cancel" onclick={() => (showDiscardConfirm = false)}>Cancel</button>
		</div>
	{/if}

	<!-- Diff content -->
	<div class="dv-content">
		{#if $isDiffLoading}
			<div class="dv-skeleton">
				{#each Array(14) as _}
					<div class="sk-line"></div>
				{/each}
			</div>
		{:else if !$diffFile}
			<div class="dv-empty">No file selected</div>
		{:else if $diffFile.hunks.length === 0}
			<div class="dv-empty">No changes to display</div>
		{:else}
			<table class="diff-table">
				<colgroup>
					<col class="col-old-no" />
					<col class="col-new-no" />
					<col class="col-prefix" />
					<col class="col-content" />
				</colgroup>
				<tbody>
					{#each $diffFile.hunks as hunk (hunk.old_start + '-' + hunk.new_start)}
						<!-- Hunk header row -->
						<tr class="hunk-header-row">
							<td colspan="3" class="hunk-header-gutter"></td>
							<td class="hunk-header-text">
								<span>{hunkHeader(hunk)}</span>
								<button class="revert-hunk-btn">Revert Hunk</button>
							</td>
						</tr>
						{#each hunk.lines as line}
							<tr
								class="diff-line"
								class:line-added={line.line_type === 'Added'}
								class:line-deleted={line.line_type === 'Deleted'}
								class:line-context={line.line_type === 'Context'}
							>
								<td class="line-no old">{line.old_line_no ?? ''}</td>
								<td class="line-no new">{line.new_line_no ?? ''}</td>
								<td class="line-prefix">
									{line.line_type === 'Added' ? '+' : line.line_type === 'Deleted' ? '-' : ' '}
								</td>
								<td class="line-content">{line.content}</td>
							</tr>
						{/each}
					{/each}
				</tbody>
			</table>
		{/if}
	</div>
</div>

<style>
	.diff-viewer {
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--bg-primary);
		overflow: hidden;
	}

	/* ── Header ── */
	.dv-header {
		display: flex;
		align-items: center;
		gap: 8px;
		height: 40px;
		min-height: 40px;
		padding: 0 12px;
		background: var(--bg-secondary);
		border-bottom: 1px solid var(--border);
		flex-shrink: 0;
	}

	.back-btn {
		display: flex;
		align-items: center;
		gap: 5px;
		padding: 4px 10px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 4px;
		color: var(--text-secondary);
		font-size: 12px;
		cursor: pointer;
		white-space: nowrap;
		flex-shrink: 0;
		transition: color 0.1s, background 0.1s;
	}
	.back-btn:hover {
		color: var(--text-primary);
		background: #2d333b;
	}

	.dv-filepath {
		flex: 1;
		display: flex;
		align-items: center;
		gap: 8px;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 12px;
		font-family: 'JetBrains Mono', 'Fira Code', monospace;
		color: var(--text-secondary);
	}

	.dv-mode-badge {
		padding: 2px 6px;
		border-radius: 3px;
		font-size: 10px;
		font-weight: 600;
		letter-spacing: 0.04em;
		flex-shrink: 0;
		font-family: -apple-system, sans-serif;
	}
	.mode-commit { background: rgba(255,255,255,0.06); color: var(--text-secondary); }
	.mode-staged { background: rgba(63,185,80,0.15); color: var(--accent-green); }
	.mode-wt { background: rgba(210,153,34,0.15); color: var(--accent-orange); }

	.dv-controls {
		display: flex;
		align-items: center;
		gap: 6px;
		flex-shrink: 0;
	}

	.toggle-group {
		display: flex;
		border: 1px solid var(--border);
		border-radius: 4px;
		overflow: hidden;
	}

	.toggle-btn {
		padding: 3px 10px;
		background: var(--bg-primary);
		border: none;
		color: var(--text-muted);
		font-size: 11px;
		cursor: pointer;
		transition: color 0.1s, background 0.1s;
	}
	.toggle-btn + .toggle-btn {
		border-left: 1px solid var(--border);
	}
	.toggle-btn:hover { color: var(--text-secondary); background: var(--bg-tertiary); }
	.toggle-btn.active { background: var(--bg-tertiary); color: var(--text-primary); }

	.close-btn {
		width: 24px;
		height: 24px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: transparent;
		border: 1px solid transparent;
		border-radius: 4px;
		color: var(--text-muted);
		font-size: 12px;
		cursor: pointer;
		transition: color 0.1s, border-color 0.1s;
	}
	.close-btn:hover {
		color: var(--accent-red);
		border-color: var(--border);
	}

	/* ── Content ── */
	.dv-content {
		flex: 1;
		overflow: auto;
		min-height: 0;
	}

	.dv-empty {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
		color: var(--text-muted);
		font-size: 13px;
	}

	.dv-skeleton {
		padding: 12px;
		display: flex;
		flex-direction: column;
		gap: 5px;
	}
	.sk-line {
		height: 14px;
		border-radius: 2px;
		background: linear-gradient(90deg, var(--bg-secondary) 0%, var(--bg-tertiary) 50%, var(--bg-secondary) 100%);
		background-size: 200% 100%;
		animation: shimmer 1.2s ease-in-out infinite;
	}
	.sk-line:nth-child(odd) { width: 100%; }
	.sk-line:nth-child(even) { width: 65%; }
	@keyframes shimmer { 0% { background-position: 200% 0; } 100% { background-position: -200% 0; } }

	/* ── Diff Table ── */
	.diff-table {
		border-collapse: collapse;
		width: 100%;
		font-family: 'JetBrains Mono', 'Fira Code', ui-monospace, monospace;
		font-size: 12px;
		line-height: 20px;
	}

	.col-old-no,
	.col-new-no { width: 40px; }
	.col-prefix  { width: 20px; }
	.col-content { width: auto; }

	.line-no {
		width: 40px;
		text-align: right;
		padding: 0 8px;
		color: var(--text-muted);
		font-size: 12px;
		user-select: none;
		border-right: 1px solid var(--bg-tertiary);
		vertical-align: top;
		white-space: nowrap;
	}

	.line-prefix {
		width: 20px;
		text-align: center;
		color: var(--text-muted);
		font-size: 12px;
		user-select: none;
		vertical-align: top;
	}

	.line-content {
		padding: 0 8px;
		white-space: pre;
		color: var(--text-primary);
		vertical-align: top;
		min-width: 0;
	}

	/* Added lines */
	.line-added { background: #0d4a23; }
	.line-added .line-prefix { color: #3fb950; }
	.line-added .line-content { color: #3fb950; }
	.line-added .line-no { color: #3fb95066; }

	/* Deleted lines */
	.line-deleted { background: #4a0d0d; }
	.line-deleted .line-prefix { color: #f85149; }
	.line-deleted .line-content { color: #f85149; }
	.line-deleted .line-no { color: #f8514966; }

	/* Context lines */
	.line-context { background: transparent; }
	.line-context .line-content { color: var(--text-secondary); }

	/* Hunk header row */
	.hunk-header-row td {
		background: #1c2128;
		border-top: 1px solid var(--border);
		border-bottom: 1px solid var(--border);
	}
	.hunk-header-gutter {
		border-right: 1px solid var(--bg-tertiary);
	}
	.hunk-header-text {
		padding: 0 8px;
		color: var(--text-muted);
		font-size: 12px;
		font-family: 'JetBrains Mono', monospace;
		display: flex;
		align-items: center;
	}

	.revert-hunk-btn {
		margin-left: auto;
		padding: 1px 8px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 3px;
		color: var(--text-muted);
		font-size: 10px;
		font-family: -apple-system, sans-serif;
		cursor: pointer;
		opacity: 0;
		transition: opacity 0.1s;
	}
	.hunk-header-row:hover .revert-hunk-btn { opacity: 1; }
	.revert-hunk-btn:hover { color: var(--accent-orange); border-color: var(--accent-orange); }

	/* Discard button */
	.discard-file-btn {
		padding: 3px 10px;
		background: transparent;
		border: 1px solid #f85149;
		border-radius: 4px;
		color: #f85149;
		font-size: 11px;
		cursor: pointer;
		transition: background 0.12s, color 0.12s;
		white-space: nowrap;
	}
	.discard-file-btn:hover {
		background: rgba(248, 81, 73, 0.15);
		color: #ff6b63;
	}

	/* Discard confirm bar */
	.discard-confirm-bar {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 6px 14px;
		background: rgba(248, 81, 73, 0.07);
		border-bottom: 1px solid rgba(248, 81, 73, 0.28);
		flex-shrink: 0;
	}
	.dc-warn {
		flex: 1;
		font-size: 11px;
		color: var(--text-secondary);
	}
	.dc-btn-danger {
		background: #f85149;
		color: white;
		border: none;
		border-radius: 3px;
		padding: 2px 12px;
		cursor: pointer;
		font-size: 11px;
		transition: opacity 0.1s;
	}
	.dc-btn-danger:hover { opacity: 0.85; }
	.dc-btn-cancel {
		background: transparent;
		color: #8b949e;
		border: 1px solid var(--border);
		border-radius: 3px;
		padding: 2px 12px;
		cursor: pointer;
		font-size: 11px;
		transition: color 0.1s;
	}
	.dc-btn-cancel:hover { color: var(--text-primary); }
</style>
