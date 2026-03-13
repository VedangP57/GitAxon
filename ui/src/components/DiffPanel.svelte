<script lang="ts">
	import {
		selectedCommit,
		selectedFile,
		commitDiffFiles,
		isLoading,
		isDiffLoading,
		selectFile
	} from '$lib/store';
	import type { DiffFile } from '$lib/types';

	function getStatusIcon(status: DiffFile['status']): { char: string; class: string } {
		switch (status) {
			case 'Added':
				return { char: 'A', class: 'icon-added' };
			case 'Deleted':
				return { char: 'D', class: 'icon-deleted' };
			case 'Modified':
			case 'Renamed':
			case 'Copied':
			default:
				return { char: 'M', class: 'icon-modified' };
		}
	}

	function getFilePath(file: DiffFile): string {
		return file.new_path ?? file.old_path ?? '';
	}

	const isFileFromCommit = $derived(
		$selectedFile &&
			$selectedCommit &&
			$commitDiffFiles.some((f) => getFilePath(f) === getFilePath($selectedFile!))
	);

	const loadingCommitDiff = $derived($selectedCommit && $isLoading);
</script>

<div class="diff-panel">
	{#if !$selectedCommit && !$selectedFile}
		<!-- State 1: No selection -->
		<div class="empty-state">
			Select a commit or file to see changes
		</div>

	{:else if $selectedCommit}
		<!-- State 2 & 3: Commit selected - show file list and/or diff -->
		<div class="diff-content">
			<div class="header">
				<div class="header-title">Changes in {$selectedCommit.commit.short_hash}</div>
				<div class="header-subtitle">{$selectedCommit.commit.message.split('\n')[0]}</div>
			</div>

			{#if loadingCommitDiff}
				<div class="loading">Loading files…</div>
			{:else if $commitDiffFiles.length > 0}
				<div class="file-list">
					{#each $commitDiffFiles as file (getFilePath(file))}
						<button
							class="file-row"
							class:selected={$selectedFile && getFilePath($selectedFile) === getFilePath(file)}
							onclick={() => selectFile(file)}
							type="button"
						>
							<span class="file-icon {getStatusIcon(file.status).class}">
								{getStatusIcon(file.status).char}
							</span>
							<span class="file-path">{getFilePath(file)}</span>
						</button>
					{/each}
				</div>
			{/if}

			{#if $selectedFile}
				<div class="diff-view">
					<div class="diff-header">
						{getFilePath($selectedFile)}
						<span class="status-badge {getStatusIcon($selectedFile.status).class}">
							{$selectedFile.status}
						</span>
					</div>
					{#if $isDiffLoading}
						<div class="diff-skeleton">
							{#each Array(8) as _}
								<div class="skeleton-line"></div>
							{/each}
						</div>
					{:else if $selectedFile.hunks.length > 0}
						<div class="diff-hunks">
							{#each $selectedFile.hunks as hunk (hunk.old_start + '-' + hunk.new_start)}
								<div class="hunk">
									<div class="hunk-header">
										@@ -{hunk.old_start},{hunk.old_lines} +{hunk.new_start},{hunk.new_lines} @@
									</div>
									{#each hunk.lines as line}
										<div class="diff-line" class:added={line.line_type === 'Added'} class:deleted={line.line_type === 'Deleted'} class:context={line.line_type === 'Context'}>
											<span class="line-nums">
												{String(line.old_line_no ?? '').padStart(6)} {String(line.new_line_no ?? '').padStart(6)}
											</span>
											<span class="line-prefix">
												{line.line_type === 'Added' ? '+' : line.line_type === 'Deleted' ? '-' : ' '}
											</span>
											<span class="line-content">{line.content}</span>
										</div>
									{/each}
								</div>
							{/each}
						</div>
					{:else}
						<div class="empty-diff">No changes to display</div>
					{/if}
				</div>
			{/if}
		</div>

	{:else if $selectedFile}
		<!-- State 3: File selected (from staging, no commit) -->
		<div class="diff-content">
			<div class="header diff-header">
				{getFilePath($selectedFile)}
				<span class="status-badge {getStatusIcon($selectedFile.status).class}">
					{$selectedFile.status}
				</span>
			</div>
			{#if $isDiffLoading}
				<div class="diff-skeleton">
					{#each Array(8) as _}
						<div class="skeleton-line"></div>
					{/each}
				</div>
			{:else if $selectedFile.hunks.length > 0}
				<div class="diff-hunks">
					{#each $selectedFile.hunks as hunk (hunk.old_start + '-' + hunk.new_start)}
						<div class="hunk">
							<div class="hunk-header">
								@@ -{hunk.old_start},{hunk.old_lines} +{hunk.new_start},{hunk.new_lines} @@
							</div>
							{#each hunk.lines as line}
								<div class="diff-line" class:added={line.line_type === 'Added'} class:deleted={line.line_type === 'Deleted'} class:context={line.line_type === 'Context'}>
									<span class="line-nums">
										{String(line.old_line_no ?? '').padStart(6)} {String(line.new_line_no ?? '').padStart(6)}
									</span>
									<span class="line-prefix">
										{line.line_type === 'Added' ? '+' : line.line_type === 'Deleted' ? '-' : ' '}
									</span>
									<span class="line-content">{line.content}</span>
								</div>
							{/each}
						</div>
					{/each}
				</div>
			{:else}
				<div class="empty-diff">No changes to display</div>
			{/if}
		</div>
	{:else}
		<div class="empty-state">No changes to display</div>
	{/if}
</div>

<style>
	.diff-panel {
		height: 100%;
		min-height: 120px;
		background: var(--bg-secondary);
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.empty-state {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--text-muted);
		font-size: 13px;
		padding: 24px;
		text-align: center;
	}

	.diff-content {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-height: 0;
		overflow: auto;
		padding: 12px;
	}

	.header {
		margin-bottom: 12px;
		padding-bottom: 8px;
		border-bottom: 1px solid var(--border);
	}

	.header-title {
		font-size: 13px;
		font-weight: 600;
		color: var(--text-primary);
	}

	.header-subtitle {
		font-size: 12px;
		color: var(--text-muted);
		margin-top: 4px;
	}

	.loading {
		color: var(--text-muted);
		font-size: 12px;
		padding: 12px;
	}

	.file-list {
		flex-shrink: 0;
		margin-bottom: 12px;
	}

	.file-row {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		padding: 6px 8px;
		font-size: 12px;
		text-align: left;
		background: transparent;
		border: none;
		border-radius: 4px;
		color: var(--text-primary);
		cursor: pointer;
		transition: background-color 100ms ease;
	}
	.file-row:hover {
		background: var(--bg-tertiary);
	}
	.file-row.selected {
		background: var(--border);
	}

	.file-icon {
		width: 18px;
		text-align: center;
		font-weight: bold;
		font-size: 11px;
		flex-shrink: 0;
	}
	.icon-added {
		color: var(--accent-green);
	}
	.icon-deleted {
		color: var(--accent-red);
	}
	.icon-modified {
		color: var(--accent-orange);
	}

	.file-path {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.diff-view {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-height: 0;
		overflow-x: auto;
		overflow-y: auto;
		max-height: 100%;
	}

	.diff-header {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 12px;
		font-weight: 600;
		margin-bottom: 8px;
		padding: 8px 0;
		border-bottom: 1px solid var(--border);
	}

	.status-badge {
		font-size: 10px;
		padding: 2px 6px;
		border-radius: 4px;
		font-weight: 500;
	}
	.status-badge.icon-added {
		background: rgba(63, 185, 80, 0.2);
		color: var(--accent-green);
	}
	.status-badge.icon-deleted {
		background: rgba(248, 81, 73, 0.2);
		color: var(--accent-red);
	}
	.status-badge.icon-modified {
		background: rgba(210, 153, 34, 0.2);
		color: var(--accent-orange);
	}

	.diff-skeleton {
		padding: 12px;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.skeleton-line {
		height: 14px;
		border-radius: 2px;
		background: linear-gradient(
			90deg,
			var(--bg-tertiary) 0%,
			var(--border) 50%,
			var(--bg-tertiary) 100%
		);
		background-size: 200% 100%;
		animation: skeleton-shimmer 1.2s ease-in-out infinite;
	}
	.skeleton-line:nth-child(odd) {
		width: 100%;
	}
	.skeleton-line:nth-child(even) {
		width: 60%;
	}
	@keyframes skeleton-shimmer {
		0% {
			background-position: 200% 0;
		}
		100% {
			background-position: -200% 0;
		}
	}

	.empty-diff {
		padding: 24px;
		color: var(--text-muted);
		font-size: 13px;
	}

	.diff-hunks {
		font-family: ui-monospace, monospace;
		font-size: 12px;
		line-height: 1.5;
		overflow-x: auto;
		overflow-y: auto;
		max-height: 100%;
		contain: layout style;
	}

	.hunk {
		margin-bottom: 12px;
	}

	.hunk-header {
		background: var(--bg-tertiary);
		color: var(--text-muted);
		padding: 4px 8px;
		font-size: 11px;
		margin-bottom: 4px;
		font-family: ui-monospace, monospace;
	}

	.diff-line {
		display: flex;
		min-width: max-content;
		padding: 0 8px;
		font-family: ui-monospace, monospace;
		font-size: 12px;
		contain: layout style;
	}
	.diff-line .line-nums {
		flex-shrink: 0;
		width: 40px;
		min-width: 40px;
		color: var(--text-muted);
		font-size: 12px;
		text-align: right;
		padding-right: 8px;
		user-select: none;
	}
	.diff-line .line-prefix {
		flex-shrink: 0;
		width: 16px;
		text-align: center;
		user-select: none;
	}
	.diff-line .line-content {
		white-space: pre;
		word-break: break-all;
	}
	.diff-line.added {
		background: #0d4a23;
		color: #3fb950;
	}
	.diff-line.added .line-prefix {
		color: #3fb950;
	}
	.diff-line.deleted {
		background: #4a0d0d;
		color: #f85149;
	}
	.diff-line.deleted .line-prefix {
		color: #f85149;
	}
	.diff-line.context {
		background: transparent;
		color: #8b949e;
	}
	.diff-line.context .line-content {
		color: #8b949e;
	}
</style>
