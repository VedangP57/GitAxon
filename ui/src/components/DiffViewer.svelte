<script lang="ts">
	import {
		diffFile,
		diffMode,
		isDiffLoading,
		closeDiff,
		currentRepo,
		selectedCommit,
		commits,
		selectCommit
	} from '$lib/store';
	import { discardFile, gitBlame, readDiffFileContent, stageHunk, unstageHunk, type BlameLine } from '$lib/tauri';
	import BlameViewer from './BlameViewer.svelte';
	import { debouncedRefreshStatus } from '$lib/store';
	import { showToast } from '$lib/toast';
	import type { DiffFile, DiffHunk, DiffLine } from '$lib/types';
	import { get } from 'svelte/store';

	let activeTab = $state<'diff' | 'file'>('diff');
	let splitMode = $state(false);

	interface SplitRow {
		left: DiffLine | null;
		right: DiffLine | null;
	}
	interface SplitHunk {
		hunk: DiffHunk;
		rows: SplitRow[];
	}

	// Pair deleted/added lines so both columns stay vertically aligned.
	// Context lines always appear on both sides. Consecutive deleted+added
	// blocks are interleaved; shorter side gets null (empty) placeholders.
	const splitRows = $derived.by((): SplitHunk[] => {
		if (!$diffFile) return [];
		return $diffFile.hunks.map((hunk) => {
			const rows: SplitRow[] = [];
			let i = 0;
			const lines = hunk.lines;
			while (i < lines.length) {
				if (lines[i].line_type === 'Context') {
					rows.push({ left: lines[i], right: lines[i] });
					i++;
				} else {
					const deleted: DiffLine[] = [];
					const added: DiffLine[] = [];
					while (i < lines.length && lines[i].line_type !== 'Context') {
						if (lines[i].line_type === 'Deleted') deleted.push(lines[i]);
						else added.push(lines[i]);
						i++;
					}
					const max = Math.max(deleted.length, added.length);
					for (let j = 0; j < max; j++) {
						rows.push({ left: deleted[j] ?? null, right: added[j] ?? null });
					}
				}
			}
			return { hunk, rows };
		});
	});

	let fileViewText = $state<string | null>(null);
	let fileViewLoading = $state(false);
	let fileViewErr = $state<string | null>(null);
	/** Latest file-view request; async results apply only if they match this key. */
	let fileViewRequestKey = $state('');
	let showDiscardConfirm = $state(false);
	let blameMode = $state(false);
	let blameData = $state<BlameLine[]>([]);
	let blameLoading = $state(false);
	let blameFileId = $state<string | null>(null); // path used to detect file switches

	function getFilePath(file: DiffFile): string {
		return file.new_path ?? file.old_path ?? '';
	}

	function hunkHeader(hunk: DiffHunk): string {
		return `@@ -${hunk.old_start},${hunk.old_lines} +${hunk.new_start},${hunk.new_lines} @@`;
	}

	// Reset blame when the displayed file changes or diff is closed
	$effect(() => {
		const file = $diffFile;
		if (!file) {
			blameMode = false;
			blameData = [];
			blameFileId = null;
			return;
		}
		if (!blameMode || !blameFileId) return;
		const currentPath = file.new_path ?? file.old_path ?? '';
		if (currentPath !== blameFileId) {
			blameMode = false;
			blameData = [];
			blameFileId = null;
		}
	});

	async function toggleBlame() {
		if (blameMode) {
			blameMode = false;
			blameData = [];
			blameFileId = null;
			return;
		}

		const repo = get(currentRepo);
		const file = get(diffFile);
		if (!repo || !file) return;

		const filePath = getFilePath(file);
		if (!filePath) return;

		const commitHash =
			get(diffMode) === 'commit' ? get(selectedCommit)?.commit.hash : undefined;

		blameLoading = true;
		try {
			blameData = await gitBlame(repo, filePath, commitHash);
			blameMode = true;
			blameFileId = filePath;
		} catch (e) {
			blameData = [];
			blameMode = false;
			blameFileId = null;
			showToast(`Blame failed: ${String(e)}`, 'error');
		} finally {
			blameLoading = false;
		}
	}

	/** Build a unified diff patch for a single hunk, suitable for `git apply --cached`. */
	function buildHunkPatch(file: DiffFile, hunk: DiffHunk): string {
		const oldPath = file.old_path ?? file.new_path ?? '';
		const newPath = file.new_path ?? file.old_path ?? '';
		const aPath = file.status === 'Added' ? '/dev/null' : `a/${oldPath}`;
		const bPath = file.status === 'Deleted' ? '/dev/null' : `b/${newPath}`;
		let patch = `diff --git a/${oldPath} b/${newPath}\n`;
		patch += `--- ${aPath}\n`;
		patch += `+++ ${bPath}\n`;
		patch += `@@ -${hunk.old_start},${hunk.old_lines} +${hunk.new_start},${hunk.new_lines} @@\n`;
		for (const line of hunk.lines) {
			const prefix = line.line_type === 'Added' ? '+' : line.line_type === 'Deleted' ? '-' : ' ';
			patch += `${prefix}${line.content}\n`;
		}
		return patch;
	}

	async function handleStageHunk(hunk: DiffHunk) {
		const file = $diffFile;
		const repo = $currentRepo;
		if (!file || !repo) return;
		try {
			const patch = buildHunkPatch(file, hunk);
			await stageHunk(repo, patch);
			showToast('Hunk staged', 'success');
			debouncedRefreshStatus();
		} catch (e) {
			showToast(`Stage hunk failed: ${String(e)}`, 'error');
		}
	}

	async function handleUnstageHunk(hunk: DiffHunk) {
		const file = $diffFile;
		const repo = $currentRepo;
		if (!file || !repo) return;
		try {
			const patch = buildHunkPatch(file, hunk);
			await unstageHunk(repo, patch);
			showToast('Hunk unstaged', 'success');
			debouncedRefreshStatus();
		} catch (e) {
			showToast(`Unstage hunk failed: ${String(e)}`, 'error');
		}
	}

	$effect(() => {
		const file = $diffFile;
		const repo = $currentRepo;
		const mode = $diffMode;
		const commit = $selectedCommit;
		const tab = activeTab;
		const path = file ? getFilePath(file) : '';

		if (tab !== 'file') {
			fileViewLoading = false;
			return;
		}
		if (!file || !repo || !path) {
			fileViewLoading = false;
			return;
		}

		const commitHash = mode === 'commit' ? commit?.commit.hash ?? null : null;
		if (mode === 'commit' && !commitHash) {
			fileViewErr = 'No commit selected';
			fileViewText = null;
			fileViewLoading = false;
			return;
		}

		const requestKey = JSON.stringify([repo, path, mode, commitHash ?? '', file.status]);
		fileViewRequestKey = requestKey;
		fileViewLoading = true;
		fileViewErr = null;
		fileViewText = null;

		void readDiffFileContent(repo, path, mode, commitHash, file.status).then(
			(text) => {
				if (fileViewRequestKey !== requestKey) return;
				fileViewText = text;
				fileViewLoading = false;
			},
			(e) => {
				if (fileViewRequestKey !== requestKey) return;
				fileViewErr = String(e);
				fileViewLoading = false;
			}
		);
	});

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
				<button class="toggle-btn bt-btn" class:active={blameMode} class:bt-active={blameMode} onclick={toggleBlame} title="Toggle blame view">Blame</button>
				<button class="toggle-btn">History</button>
			</div>
			<button
				class="toolbar-btn"
				class:active={splitMode}
				onclick={() => (splitMode = !splitMode)}
				title="Toggle split view"
			>⇔ Split</button>
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
				{#each Array(14) as _, i (i)}
					<div class="sk-line"></div>
				{/each}
			</div>
		{:else if !$diffFile}
			<div class="dv-empty">No file selected</div>
		{:else if blameMode}
			<div class="blame-view">
				{#if blameLoading}
					<div class="blame-loading">Loading blame...</div>
				{:else if blameData.length === 0}
					<div class="dv-empty">No blame data available</div>
				{:else}
					<BlameViewer
						lines={blameData}
						onhashclick={(hash) => {
							const commitList = get(commits);
							const found = commitList.find((c) => c.commit.hash.startsWith(hash));
							if (found) selectCommit(found);
						}}
					/>
				{/if}
			</div>
		{:else if activeTab === 'file'}
			<div class="file-view-pane">
				{#if fileViewLoading}
					<div class="blame-loading">Loading file…</div>
				{:else if fileViewErr}
					<div class="dv-empty file-view-err">{fileViewErr}</div>
				{:else if fileViewText !== null}
					<pre class="file-view-pre">{fileViewText}</pre>
				{/if}
			</div>
		{:else if $diffFile.hunks.length === 0}
			<div class="dv-empty">No changes to display</div>
		{:else if splitMode}
			<div class="split-grid">
				{#each splitRows as { hunk, rows } (hunk.old_start + '-' + hunk.new_start)}
					<div class="split-hunk-header">@@ -{hunk.old_start},{hunk.old_lines} +{hunk.new_start},{hunk.new_lines} @@</div>
					{#each rows as row, i (i)}
						<div class="split-cell {row.left ? (row.left.line_type === 'Deleted' ? 'line-deleted' : 'line-context') : 'line-empty'}">
							{#if row.left}
								<span class="sp-ln">{row.left.old_line_no ?? ''}</span>
								<span class="sp-content">{row.left.content}</span>
							{:else}
								<span class="sp-ln"></span><span class="sp-content"></span>
							{/if}
						</div>
						<div class="split-cell {row.right ? (row.right.line_type === 'Added' ? 'line-added' : 'line-context') : 'line-empty'}">
							{#if row.right}
								<span class="sp-ln">{row.right.new_line_no ?? ''}</span>
								<span class="sp-content">{row.right.content}</span>
							{:else}
								<span class="sp-ln"></span><span class="sp-content"></span>
							{/if}
						</div>
					{/each}
				{/each}
			</div>
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
								{#if $diffMode === 'working-tree'}
									<button class="stage-hunk-btn" onclick={() => handleStageHunk(hunk)}>Stage Hunk</button>
								{:else if $diffMode === 'staged'}
									<button class="unstage-hunk-btn" onclick={() => handleUnstageHunk(hunk)}>Unstage Hunk</button>
								{/if}
							</td>
						</tr>
						{#each hunk.lines as line, i (`${line.old_line_no ?? 'x'}-${line.new_line_no ?? 'x'}-${line.content}-${i}`)}
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
	.bt-btn {
		position: relative;
	}
	.bt-active {
		background: var(--bg-tertiary);
		color: var(--text-primary);
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

	.file-view-pane {
		height: 100%;
		min-height: 120px;
	}
	.file-view-pre {
		margin: 0;
		padding: 12px 14px;
		font-family: 'JetBrains Mono', 'Fira Code', ui-monospace, monospace;
		font-size: 12px;
		line-height: 20px;
		white-space: pre;
		color: var(--text-primary);
		tab-size: 4;
	}
	.file-view-err {
		padding: 16px;
		text-align: center;
		color: var(--accent-orange);
		font-size: 12px;
		white-space: pre-wrap;
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
	.blame-view {
		position: relative;
		height: 100%;
	}
	.blame-loading {
		padding: 14px;
		color: var(--text-secondary);
		font-size: 12px;
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

	.stage-hunk-btn, .unstage-hunk-btn {
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
	.hunk-header-row:hover .stage-hunk-btn,
	.hunk-header-row:hover .unstage-hunk-btn { opacity: 1; }
	.stage-hunk-btn:hover { color: var(--accent-green); border-color: var(--accent-green); }
	.unstage-hunk-btn:hover { color: var(--accent-orange); border-color: var(--accent-orange); }

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

	/* ── Toolbar split button ── */
	.toolbar-btn {
		padding: 3px 10px;
		background: var(--bg-primary);
		border: 1px solid var(--border);
		border-radius: 4px;
		color: var(--text-muted);
		font-size: 11px;
		cursor: pointer;
		transition: color 0.1s, background 0.1s;
		white-space: nowrap;
	}
	.toolbar-btn:hover { color: var(--text-secondary); background: var(--bg-tertiary); }
	.toolbar-btn.active { background: var(--bg-tertiary); color: var(--text-primary); border-color: var(--accent-blue); }

	/* ── Split view (CSS grid — single scroll container, aligned rows) ── */
	.split-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		overflow: auto;
		height: 100%;
		font-family: 'JetBrains Mono', 'Fira Code', ui-monospace, monospace;
		font-size: 12px;
		line-height: 20px;
	}
	.split-hunk-header {
		grid-column: 1 / -1;
		padding: 2px 8px;
		background: #1c2128;
		border-top: 1px solid var(--border);
		border-bottom: 1px solid var(--border);
		color: var(--text-muted);
		font-size: 12px;
		white-space: pre;
	}
	.split-cell {
		display: flex;
		align-items: baseline;
		min-width: 0;
		border-right: 1px solid var(--border);
	}
	.split-cell:nth-child(even) { border-right: none; }
	.split-cell.line-deleted { background: #4a0d0d; }
	.split-cell.line-added   { background: #0d4a23; }
	.split-cell.line-context { background: transparent; }
	.split-cell.line-empty   { background: var(--bg-secondary); opacity: 0.5; }
	.sp-ln {
		min-width: 40px;
		padding: 0 8px;
		text-align: right;
		color: var(--text-muted);
		user-select: none;
		flex-shrink: 0;
		border-right: 1px solid var(--bg-tertiary);
	}
	.sp-content {
		padding: 0 8px;
		white-space: pre;
		color: var(--text-primary);
	}
	.split-cell.line-deleted .sp-content { color: #f85149; }
	.split-cell.line-added   .sp-content { color: #3fb950; }
	.split-cell.line-context .sp-content { color: var(--text-secondary); }
</style>
