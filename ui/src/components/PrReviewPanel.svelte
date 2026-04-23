<script lang="ts">
	import { get } from 'svelte/store';
	import { currentRepo, selectedPr, closeDiff, prs } from '$lib/store';
	import { getPrComments, getPrFiles, submitPrReview } from '$lib/tauri';
	import { showToast } from '$lib/toast';
	import type { ReviewComment, PrFile, PullRequest } from '$lib/types';

	let prNumber = $state<number | null>(null);
	let pr = $state<PullRequest | null>(null);
	let files = $state<PrFile[]>([]);
	let comments = $state<ReviewComment[]>([]);
	let isLoading = $state(false);
	let reviewBody = $state('');
	let selectedFile = $state<PrFile | null>(null);

	$effect(() => {
		const unsub = selectedPr.subscribe(async (num) => {
			prNumber = num;
			if (num) await loadPrData(num);
		});
		return unsub;
	});

	async function loadPrData(num: number) {
		const repo = get(currentRepo);
		if (!repo) return;

		// Find PR details from store
		const prList = get(prs);
		pr = prList.find(p => p.number === num) ?? null;

		isLoading = true;
		try {
			const [f, c] = await Promise.all([
				getPrFiles(repo, num),
				getPrComments(repo, num),
			]);
			files = f;
			comments = c;
		} catch (e) {
			showToast(String(e), 'error');
		} finally {
			isLoading = false;
		}
	}

	function commentsForFile(path: string): ReviewComment[] {
		return comments.filter(c => c.path === path);
	}

	async function handleSubmitReview(event: string) {
		const repo = get(currentRepo);
		if (!repo || !prNumber) return;
		try {
			await submitPrReview(repo, prNumber, reviewBody, event);
			showToast(`Review submitted: ${event.toLowerCase()}`, 'success');
			reviewBody = '';
		} catch (e) {
			showToast(String(e), 'error');
		}
	}

	function formatDate(dateStr: string): string {
		const d = new Date(dateStr);
		return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
	}
</script>

<div class="pr-review">
	<div class="pr-header">
		<button class="pr-back" onclick={() => closeDiff()}>←</button>
		<div class="pr-title-area">
			{#if pr}
				<span class="pr-number">#{pr.number}</span>
				<span class="pr-title">{pr.title}</span>
				<div class="pr-meta">
					<span>{pr.author}</span>
					<span class="pr-arrow">{pr.headBranch} → {pr.baseBranch}</span>
					{#if pr.draft}<span class="pr-draft">DRAFT</span>{/if}
					<span class="pr-stats">+{pr.additions} -{pr.deletions}</span>
				</div>
			{:else}
				<span class="pr-title">PR #{prNumber}</span>
			{/if}
		</div>
	</div>

	{#if isLoading}
		<div class="pr-loading">Loading PR data...</div>
	{:else}
		<div class="pr-content">
			<div class="pr-file-list">
				<div class="pr-section-label">FILES CHANGED ({files.length})</div>
				{#each files as file (file.filename)}
					<button
						class="pr-file"
						class:active={selectedFile?.filename === file.filename}
						onclick={() => (selectedFile = file)}
					>
						<span class="pr-file-status" class:added={file.status === 'added'} class:removed={file.status === 'removed'}>
							{file.status === 'added' ? 'A' : file.status === 'removed' ? 'D' : 'M'}
						</span>
						<span class="pr-file-name">{file.filename.split('/').pop()}</span>
						<span class="pr-file-path">{file.filename}</span>
						<span class="pr-file-stats">+{file.additions} -{file.deletions}</span>
						{#if commentsForFile(file.filename).length > 0}
							<span class="pr-file-comments">{commentsForFile(file.filename).length}</span>
						{/if}
					</button>
				{/each}
			</div>

			<div class="pr-detail">
				{#if selectedFile}
					<div class="pr-detail-header">
						<span class="pr-detail-name">{selectedFile.filename}</span>
					</div>

					{#if commentsForFile(selectedFile.filename).length > 0}
						<div class="pr-comments-section">
							{#each commentsForFile(selectedFile.filename) as comment (comment.id)}
								<div class="pr-comment">
									<div class="pr-comment-header">
										<span class="pr-comment-author">{comment.author}</span>
										<span class="pr-comment-date">{formatDate(comment.createdAt)}</span>
										{#if comment.line}<span class="pr-comment-line">L{comment.line}</span>{/if}
									</div>
									<div class="pr-comment-body">{comment.body}</div>
								</div>
							{/each}
						</div>
					{/if}

					{#if selectedFile.patch}
						<pre class="pr-patch">{selectedFile.patch}</pre>
					{:else}
						<div class="pr-no-patch">Binary file or no diff available</div>
					{/if}
				{:else}
					<div class="pr-select-file">Select a file to view</div>
				{/if}
			</div>
		</div>

		<div class="pr-review-form">
			<textarea
				class="pr-review-textarea"
				placeholder="Leave a review comment..."
				bind:value={reviewBody}
				rows="3"
			></textarea>
			<div class="pr-review-actions">
				<button class="pr-review-btn comment" onclick={() => handleSubmitReview('COMMENT')}>Comment</button>
				<button class="pr-review-btn approve" onclick={() => handleSubmitReview('APPROVE')}>Approve</button>
				<button class="pr-review-btn request-changes" onclick={() => handleSubmitReview('REQUEST_CHANGES')}>Request Changes</button>
			</div>
		</div>
	{/if}
</div>

<style>
	.pr-review {
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--bg-primary);
	}

	.pr-header {
		display: flex;
		align-items: flex-start;
		gap: 8px;
		padding: 8px 12px;
		border-bottom: 1px solid var(--border);
		flex-shrink: 0;
	}

	.pr-back {
		background: none;
		border: none;
		color: var(--text-secondary);
		cursor: pointer;
		font-size: 16px;
		padding: 4px 8px;
		border-radius: 4px;
	}
	.pr-back:hover { background: var(--bg-secondary); }

	.pr-title-area { flex: 1; }
	.pr-number { color: var(--text-muted); font-size: 12px; margin-right: 4px; }
	.pr-title { font-size: 13px; font-weight: 600; color: var(--text-primary); }
	.pr-meta { font-size: 11px; color: var(--text-muted); display: flex; gap: 8px; margin-top: 2px; }
	.pr-arrow { color: var(--accent-blue); }
	.pr-draft { color: var(--accent-orange); font-weight: 600; font-size: 10px; }
	.pr-stats { font-family: monospace; }

	.pr-loading, .pr-select-file, .pr-no-patch {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--text-muted);
		font-size: 13px;
	}

	.pr-content {
		flex: 1;
		display: flex;
		overflow: hidden;
	}

	.pr-file-list {
		width: 280px;
		border-right: 1px solid var(--border);
		overflow-y: auto;
		flex-shrink: 0;
	}

	.pr-section-label {
		padding: 6px 12px;
		font-size: 10px;
		font-weight: 600;
		color: var(--text-muted);
		letter-spacing: 0.5px;
	}

	.pr-file {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 5px 12px;
		border: none;
		background: transparent;
		cursor: pointer;
		width: 100%;
		text-align: left;
		font-size: 12px;
		color: var(--text-secondary);
	}
	.pr-file:hover { background: var(--bg-secondary); }
	.pr-file.active { background: var(--bg-tertiary); color: var(--text-primary); }

	.pr-file-status {
		width: 14px;
		height: 14px;
		border-radius: 2px;
		font-size: 9px;
		font-weight: 700;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		color: var(--accent-orange);
	}
	.pr-file-status.added { color: var(--accent-green); }
	.pr-file-status.removed { color: var(--accent-red); }

	.pr-file-name { font-weight: 600; color: var(--text-primary); flex-shrink: 0; }
	.pr-file-path {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--text-muted);
		font-size: 10px;
	}
	.pr-file-stats { font-family: monospace; font-size: 10px; color: var(--text-muted); flex-shrink: 0; }
	.pr-file-comments {
		background: var(--accent-blue);
		color: white;
		border-radius: 8px;
		padding: 0 5px;
		font-size: 9px;
		font-weight: 700;
		flex-shrink: 0;
	}

	.pr-detail {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.pr-detail-header {
		padding: 6px 12px;
		border-bottom: 1px solid var(--border);
		font-size: 12px;
		font-family: monospace;
		color: var(--accent-blue);
	}

	.pr-comments-section {
		border-bottom: 1px solid var(--border);
		max-height: 200px;
		overflow-y: auto;
	}

	.pr-comment {
		padding: 8px 12px;
		border-bottom: 1px solid var(--border);
	}

	.pr-comment-header {
		display: flex;
		gap: 8px;
		font-size: 11px;
		margin-bottom: 4px;
	}
	.pr-comment-author { font-weight: 600; color: var(--text-primary); }
	.pr-comment-date { color: var(--text-muted); }
	.pr-comment-line { color: var(--accent-orange); font-family: monospace; }
	.pr-comment-body { font-size: 12px; color: var(--text-secondary); line-height: 1.4; white-space: pre-wrap; }

	.pr-patch {
		flex: 1;
		overflow: auto;
		padding: 8px 12px;
		margin: 0;
		font-size: 11px;
		font-family: 'JetBrains Mono', monospace;
		line-height: 1.5;
		color: var(--text-primary);
		white-space: pre;
		tab-size: 4;
	}

	.pr-review-form {
		border-top: 1px solid var(--border);
		padding: 8px 12px;
		flex-shrink: 0;
	}

	.pr-review-textarea {
		width: 100%;
		padding: 6px 8px;
		font-size: 12px;
		background: var(--bg-secondary);
		border: 1px solid var(--border);
		border-radius: 4px;
		color: var(--text-primary);
		resize: vertical;
		font-family: inherit;
	}
	.pr-review-textarea:focus { outline: none; border-color: var(--accent-blue); }

	.pr-review-actions {
		display: flex;
		gap: 6px;
		justify-content: flex-end;
		margin-top: 6px;
	}

	.pr-review-btn {
		padding: 4px 12px;
		font-size: 11px;
		border: 1px solid var(--border);
		border-radius: 3px;
		cursor: pointer;
		background: var(--bg-secondary);
		color: var(--text-secondary);
	}
	.pr-review-btn:hover { background: var(--bg-tertiary); }
	.pr-review-btn.approve { background: var(--accent-green); color: white; border: none; }
	.pr-review-btn.request-changes { color: var(--accent-red); border-color: var(--accent-red); }
	.pr-review-btn.comment { color: var(--accent-blue); border-color: var(--accent-blue); }
</style>
