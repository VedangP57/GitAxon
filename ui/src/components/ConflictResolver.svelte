<script lang="ts">
	import { get } from 'svelte/store';
	import { currentRepo, closeDiff } from '$lib/store';
	import { getConflictFile, resolveConflict, continueOperation, abortOperation } from '$lib/tauri';
	import { showToast } from '$lib/toast';
	import type { ConflictFile } from '$lib/types';

	let conflictFile = $state<ConflictFile | null>(null);
	let resolvedContent = $state('');
	let isLoading = $state(false);
	let filePath = $state('');

	export async function loadConflict(path: string) {
		const repo = get(currentRepo);
		if (!repo) return;
		filePath = path;
		isLoading = true;
		try {
			conflictFile = await getConflictFile(repo, path);
			resolvedContent = conflictFile.merged;
		} catch (e) {
			showToast(String(e), 'error');
		} finally {
			isLoading = false;
		}
	}

	function acceptOurs() {
		if (conflictFile) resolvedContent = conflictFile.ours;
	}

	function acceptTheirs() {
		if (conflictFile) resolvedContent = conflictFile.theirs;
	}

	async function markResolved() {
		const repo = get(currentRepo);
		if (!repo || !filePath) return;
		try {
			await resolveConflict(repo, filePath, resolvedContent);
			showToast(`Resolved: ${filePath}`, 'success');
			closeDiff();
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	async function handleContinue() {
		const repo = get(currentRepo);
		if (!repo) return;
		try {
			const msg = await continueOperation(repo);
			showToast(msg || 'Operation continued', 'success');
			closeDiff();
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	async function handleAbort() {
		const repo = get(currentRepo);
		if (!repo) return;
		if (!confirm('Abort the current operation? All progress will be lost.')) return;
		try {
			const msg = await abortOperation(repo);
			showToast(msg, 'success');
			closeDiff();
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	function lineCount(text: string): number {
		return text.split('\n').length;
	}
</script>

<div class="conflict-resolver">
	<div class="cr-header">
		<button class="cr-back" onclick={() => closeDiff()}>←</button>
		<div class="cr-title">
			<span class="cr-label">CONFLICT RESOLUTION</span>
			<span class="cr-path">{filePath}</span>
		</div>
		<div class="cr-actions">
			<button class="cr-btn accept-ours" onclick={acceptOurs} title="Use our version">Accept Ours</button>
			<button class="cr-btn accept-theirs" onclick={acceptTheirs} title="Use their version">Accept Theirs</button>
			<button class="cr-btn resolve" onclick={markResolved}>Mark Resolved</button>
			<button class="cr-btn continue-op" onclick={handleContinue}>Continue</button>
			<button class="cr-btn abort-op" onclick={handleAbort}>Abort</button>
		</div>
	</div>

	{#if isLoading}
		<div class="cr-loading">Loading conflict data...</div>
	{:else if conflictFile}
		<div class="cr-panes">
			<div class="cr-pane">
				<div class="pane-header ours">OURS (current)</div>
				<pre class="pane-content">{conflictFile.ours}</pre>
			</div>
			<div class="cr-pane">
				<div class="pane-header theirs">THEIRS (incoming)</div>
				<pre class="pane-content">{conflictFile.theirs}</pre>
			</div>
			<div class="cr-pane result">
				<div class="pane-header result-header">RESULT</div>
				<textarea
					class="pane-editor"
					bind:value={resolvedContent}
					rows={lineCount(resolvedContent)}
				></textarea>
			</div>
		</div>
	{:else}
		<div class="cr-empty">Select a conflicted file to resolve</div>
	{/if}
</div>

<style>
	.conflict-resolver {
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--bg-primary);
	}

	.cr-header {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 12px;
		border-bottom: 1px solid var(--border);
		flex-shrink: 0;
	}

	.cr-back {
		background: none;
		border: none;
		color: var(--text-secondary);
		cursor: pointer;
		font-size: 16px;
		padding: 4px 8px;
		border-radius: 4px;
	}

	.cr-back:hover {
		background: var(--bg-secondary);
	}

	.cr-title {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.cr-label {
		font-size: 10px;
		font-weight: 600;
		letter-spacing: 0.5px;
		color: var(--text-muted);
	}

	.cr-path {
		font-size: 12px;
		color: var(--accent-orange);
		font-family: monospace;
	}

	.cr-actions {
		display: flex;
		gap: 4px;
	}

	.cr-btn {
		padding: 4px 10px;
		font-size: 11px;
		border: 1px solid var(--border);
		border-radius: 3px;
		cursor: pointer;
		background: var(--bg-secondary);
		color: var(--text-secondary);
	}

	.cr-btn:hover {
		background: var(--bg-tertiary);
		color: var(--text-primary);
	}

	.accept-ours { border-color: var(--accent-blue); color: var(--accent-blue); }
	.accept-theirs { border-color: var(--accent-green); color: var(--accent-green); }
	.resolve { background: var(--accent-green); color: white; border: none; }
	.continue-op { background: var(--accent-blue); color: white; border: none; }
	.abort-op { color: var(--accent-red); border-color: var(--accent-red); }

	.cr-loading, .cr-empty {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--text-muted);
		font-size: 13px;
	}

	.cr-panes {
		flex: 1;
		display: grid;
		grid-template-columns: 1fr 1fr;
		grid-template-rows: 1fr 1fr;
		gap: 1px;
		background: var(--border);
		overflow: hidden;
	}

	.cr-pane {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		background: var(--bg-primary);
	}

	.cr-pane.result {
		grid-column: 1 / -1;
	}

	.pane-header {
		padding: 4px 10px;
		font-size: 10px;
		font-weight: 700;
		letter-spacing: 0.5px;
		border-bottom: 1px solid var(--border);
		flex-shrink: 0;
	}

	.pane-header.ours { color: var(--accent-blue); background: rgba(88, 166, 255, 0.05); }
	.pane-header.theirs { color: var(--accent-green); background: rgba(63, 185, 80, 0.05); }
	.pane-header.result-header { color: var(--accent-orange); background: rgba(210, 153, 34, 0.05); }

	.pane-content {
		flex: 1;
		overflow: auto;
		padding: 8px 12px;
		margin: 0;
		font-size: 12px;
		font-family: 'JetBrains Mono', monospace;
		line-height: 1.5;
		color: var(--text-primary);
		white-space: pre;
		tab-size: 4;
	}

	.pane-editor {
		flex: 1;
		resize: none;
		padding: 8px 12px;
		border: none;
		background: var(--bg-primary);
		color: var(--text-primary);
		font-size: 12px;
		font-family: 'JetBrains Mono', monospace;
		line-height: 1.5;
		tab-size: 4;
	}

	.pane-editor:focus {
		outline: none;
		background: var(--bg-secondary);
	}
</style>
