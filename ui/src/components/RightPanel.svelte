<script lang="ts">
	import { onMount } from "svelte";
	import {
		currentRepo,
		status,
		rightPanelMode,
		selectedCommit,
		commitDiffFiles,
		isLoading,
		refreshStatus,
		loadRepo,
		selectFile,
		selectFileFromStaging,
	} from "$lib/store";
	import {
		stageFile,
		unstageFile,
		stageAll,
		unstageAll,
		createCommit,
		getGitConfig,
	} from "$lib/tauri";
	import { showToast } from "$lib/toast";
	import type { IndexEntry, DiffFile } from "$lib/types";

	const unstagedStatuses = ["Unstaged", "Untracked", "Conflicted"] as const;

	const unstagedFiles = $derived(
		$status.filter((e) =>
			unstagedStatuses.includes(
				e.status as (typeof unstagedStatuses)[number],
			),
		),
	);
	const stagedFiles = $derived($status.filter((e) => e.status === "Staged"));

	let unstagedOpen = $state(true);
	let stagedOpen = $state(true);
	let commitMessage = $state("");
	let description = $state("");
	let amend = $state(false);
	let isCommitting = $state(false);
	let isStaging = $state<string | null>(null);
	let authorName = $state("");
	let authorEmail = $state("");

	const subjectLength = $derived(commitMessage.length);
	const canCommit = $derived(
		commitMessage.trim().length > 0 && stagedFiles.length > 0,
	);

	$effect(() => {
		const repo = $currentRepo;
		if (repo) {
			Promise.all([
				getGitConfig(repo, "user.name").catch(() => ""),
				getGitConfig(repo, "user.email").catch(() => ""),
			]).then(([name, email]) => {
				authorName = name;
				authorEmail = email;
			});
		}
	});

	function getStatusIcon(entry: IndexEntry): { char: string; cls: string } {
		switch (entry.status) {
			case "Untracked":
				return { char: "A", cls: "icon-add" };
			case "Conflicted":
				return { char: "!", cls: "icon-conflict" };
			case "Staged":
			case "Unstaged":
			default:
				return { char: "M", cls: "icon-mod" };
		}
	}

	function getCommitFileIcon(file: DiffFile): { char: string; cls: string } {
		switch (file.status) {
			case "Added":
				return { char: "A", cls: "icon-add" };
			case "Deleted":
				return { char: "D", cls: "icon-del" };
			default:
				return { char: "M", cls: "icon-mod" };
		}
	}

	function fileName(path: string): string {
		return path.split("/").pop() ?? path;
	}

	function dirName(path: string): string {
		const parts = path.split("/");
		return parts.length > 1 ? parts.slice(0, -1).join("/") : "";
	}

	function getFilePath(file: DiffFile): string {
		return file.new_path ?? file.old_path ?? "";
	}

	async function handleStage(entry: IndexEntry, e: MouseEvent) {
		e.stopPropagation();
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

	async function handleUnstage(entry: IndexEntry, e: MouseEvent) {
		e.stopPropagation();
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

	async function handleStageAll() {
		const repo = $currentRepo;
		if (!repo) return;
		isStaging = "__all__";
		try {
			await stageAll(repo);
			await refreshStatus();
		} finally {
			isStaging = null;
		}
	}

	async function handleUnstageAll() {
		const repo = $currentRepo;
		if (!repo) return;
		isStaging = "__all__";
		try {
			await unstageAll(repo);
			await refreshStatus();
		} finally {
			isStaging = null;
		}
	}

	async function handleUnstagedClick(entry: IndexEntry) {
		const repo = $currentRepo;
		if (!repo) return;
		await selectFileFromStaging(repo, entry.path, false);
	}

	async function handleStagedClick(entry: IndexEntry) {
		const repo = $currentRepo;
		if (!repo) return;
		await selectFileFromStaging(repo, entry.path, true);
	}

	function handleCommitFileClick(file: DiffFile) {
		selectFile(file, "commit");
	}

	async function handleCommit() {
		const repo = $currentRepo;
		if (!repo || !canCommit) return;
		isCommitting = true;
		try {
			await createCommit(
				repo,
				commitMessage.trim(),
				authorName,
				authorEmail,
			);
			await loadRepo(repo);
			commitMessage = "";
			description = "";
			showToast("Commit created successfully", "success");
		} catch (e) {
			showToast(e instanceof Error ? e.message : String(e), "error");
		} finally {
			isCommitting = false;
		}
	}

	// Relative time
	function relativeTime(ts: number): string {
		const diff = Date.now() / 1000 - ts;
		if (diff < 60) return "just now";
		if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
		if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
		return `${Math.floor(diff / 86400)}d ago`;
	}

	function authorInitials(name: string): string {
		return name
			.split(" ")
			.map((w) => w[0])
			.join("")
			.slice(0, 2)
			.toUpperCase();
	}

	const commitSummary = $derived(() => {
		if (!$commitDiffFiles.length) return "";
		const modified = $commitDiffFiles.filter(
			(f) =>
				f.status === "Modified" ||
				f.status === "Renamed" ||
				f.status === "Copied",
		).length;
		const added = $commitDiffFiles.filter(
			(f) => f.status === "Added",
		).length;
		const deleted = $commitDiffFiles.filter(
			(f) => f.status === "Deleted",
		).length;
		const parts: string[] = [];
		if (modified) parts.push(`${modified} modified`);
		if (added) parts.push(`${added} added`);
		if (deleted) parts.push(`${deleted} deleted`);
		return parts.join(" + ");
	});
</script>

<div class="right-panel">
	{#if $rightPanelMode === "wip"}
		<!-- ══════════════ WIP MODE ══════════════ -->
		<div class="rp-header">
			<span class="wip-label">// WIP</span>
			<span class="file-count-badge">{$status.length}</span>
			<button
				class="hdr-action-btn green"
				onclick={handleStageAll}
				disabled={unstagedFiles.length === 0 || isStaging !== null}
				title="Stage all changes"
			>
				Stage All
			</button>
		</div>

		<div class="rp-scrollable">
			<!-- UNSTAGED section -->
			<div class="file-section">
				<div class="section-header">
					<button
						class="collapse-btn"
						onclick={() => (unstagedOpen = !unstagedOpen)}
					>
						<span class="chevron">{unstagedOpen ? "▼" : "▶"}</span>
						<span>Unstaged Files</span>
						<span class="count-badge">{unstagedFiles.length}</span>
					</button>
					{#if unstagedFiles.length > 0}
						<button
							class="inline-action"
							onclick={handleStageAll}
							disabled={isStaging !== null}>Stage All</button
						>
					{/if}
				</div>
				{#if unstagedOpen}
					<div class="file-list">
						{#if unstagedFiles.length === 0}
							<div class="empty-files">Working tree clean</div>
						{:else}
							{#each unstagedFiles as entry (entry.path)}
								<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
								<div
									class="file-row"
									onclick={() => handleUnstagedClick(entry)}
								>
									<span
										class="status-icon {getStatusIcon(entry)
											.cls}"
										>{getStatusIcon(entry).char}</span
									>
									<span class="file-name"
										>{fileName(entry.path)}</span
									>
									{#if dirName(entry.path)}
										<span class="file-dir"
											>{dirName(entry.path)}</span
										>
									{/if}
									<button
										class="stage-btn plus"
										onclick={(e) => handleStage(entry, e)}
										disabled={isStaging !== null}
										title="Stage {entry.path}">+</button
									>
								</div>
							{/each}
						{/if}
					</div>
				{/if}
			</div>

			<!-- STAGED section -->
			<div class="file-section">
				<div class="section-header">
					<button
						class="collapse-btn"
						onclick={() => (stagedOpen = !stagedOpen)}
					>
						<span class="chevron">{stagedOpen ? "▼" : "▶"}</span>
						<span>Staged Files</span>
						<span class="count-badge">{stagedFiles.length}</span>
					</button>
					{#if stagedFiles.length > 0}
						<button
							class="inline-action"
							onclick={handleUnstageAll}
							disabled={isStaging !== null}>Unstage All</button
						>
					{/if}
				</div>
				{#if stagedOpen}
					<div class="file-list">
						{#if stagedFiles.length === 0}
							<div class="empty-files">Nothing staged</div>
						{:else}
							{#each stagedFiles as entry (entry.path)}
								<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
								<div
									class="file-row"
									onclick={() => handleStagedClick(entry)}
								>
									<span
										class="status-icon {getStatusIcon(entry)
											.cls}"
										>{getStatusIcon(entry).char}</span
									>
									<span class="file-name"
										>{fileName(entry.path)}</span
									>
									{#if dirName(entry.path)}
										<span class="file-dir"
											>{dirName(entry.path)}</span
										>
									{/if}
									<button
										class="stage-btn minus"
										onclick={(e) => handleUnstage(entry, e)}
										disabled={isStaging !== null}
										title="Unstage {entry.path}">−</button
									>
								</div>
							{/each}
						{/if}
					</div>
				{/if}
			</div>
		</div>

		<!-- Fixed commit box -->
		<div class="commit-box">
			<div class="commit-tabs">
				<button class="tab-btn active">Commit</button>
				<label class="amend-label">
					<input type="checkbox" bind:checked={amend} />
					Amend
				</label>
			</div>

			<div class="textarea-wrap">
				<textarea
					class="commit-textarea"
					placeholder="Commit summary"
					bind:value={commitMessage}
					rows="2"
				></textarea>
				{#if subjectLength > 0}
					<span class="char-counter" class:over={subjectLength > 72}
						>{subjectLength}</span
					>
				{/if}
			</div>

			<textarea
				class="commit-textarea desc-textarea"
				placeholder="Description (optional)"
				bind:value={description}
				rows="2"
			></textarea>

			<div class="commit-footer">
				<button class="commit-opts-btn">Commit options ▾</button>
				<button
					class="commit-btn"
					disabled={!canCommit || isCommitting}
					onclick={handleCommit}
				>
					{#if canCommit}
						{isCommitting ? "Committing..." : "Commit Changes"}
					{:else}
						← Stage Changes to Commit
					{/if}
				</button>
			</div>
		</div>
	{:else}
		<!-- ══════════════ COMMIT MODE ══════════════ -->
		{#if $selectedCommit}
			<div class="rp-header commit-mode-header">
				<span class="commit-hash-label"
					>commit: {$selectedCommit.commit.short_hash}</span
				>
				<button class="explain-btn">Explain commit</button>
			</div>

			<div class="rp-scrollable">
				<!-- Author info -->
				<div class="author-section">
					<div
						class="author-avatar"
						title={$selectedCommit.commit.author_name}
					>
						{authorInitials($selectedCommit.commit.author_name)}
					</div>
					<div class="author-details">
						<span class="author-name"
							>{$selectedCommit.commit.author_name}</span
						>
						<span class="author-meta"
							>authored {relativeTime(
								$selectedCommit.commit.timestamp,
							)}</span
						>
					</div>
				</div>

				<div class="commit-message-display">
					{$selectedCommit.commit.message.split("\n")[0]}
				</div>

				{#if $commitDiffFiles.length > 0}
					<div class="commit-summary-line">
						{commitSummary()}
					</div>
				{/if}

				{#if $isLoading}
					<div class="loading-files">
						{#each Array(5) as _}
							<div class="sk-file-row"></div>
						{/each}
					</div>
				{:else}
					<div class="commit-file-list">
						{#each $commitDiffFiles as file (getFilePath(file))}
							<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
							<div
								class="file-row commit-file-row"
								onclick={() => handleCommitFileClick(file)}
							>
								<span
									class="status-icon {getCommitFileIcon(file)
										.cls}"
									>{getCommitFileIcon(file).char}</span
								>
								<span class="file-name"
									>{fileName(getFilePath(file))}</span
								>
								{#if dirName(getFilePath(file))}
									<span class="file-dir"
										>{dirName(getFilePath(file))}</span
									>
								{/if}
							</div>
						{/each}
					</div>
				{/if}
			</div>
		{/if}
	{/if}
</div>

<style>
	.right-panel {
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--bg-secondary);
		overflow: hidden;
	}

	/* ── Header ── */
	.rp-header {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 0 12px;
		height: 40px;
		min-height: 40px;
		background: var(--bg-secondary);
		border-bottom: 1px solid var(--border);
		flex-shrink: 0;
	}

	.wip-label {
		font-size: 12px;
		font-weight: 700;
		color: var(--accent-green);
		font-family: "JetBrains Mono", monospace;
	}

	.file-count-badge {
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 10px;
		padding: 1px 7px;
		font-size: 11px;
		color: var(--text-secondary);
	}

	.hdr-action-btn {
		margin-left: auto;
		padding: 3px 10px;
		font-size: 11px;
		border-radius: 4px;
		cursor: pointer;
		border: 1px solid var(--border);
		background: var(--bg-tertiary);
		color: var(--text-secondary);
		transition:
			color 0.1s,
			border-color 0.1s;
	}
	.hdr-action-btn:hover:not(:disabled) {
		color: var(--text-primary);
	}
	.hdr-action-btn.green {
		color: var(--accent-green);
		border-color: var(--accent-green);
	}
	.hdr-action-btn.green:hover:not(:disabled) {
		background: rgba(63, 185, 80, 0.1);
	}
	.hdr-action-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	/* ── Scrollable area ── */
	.rp-scrollable {
		flex: 1;
		overflow-y: auto;
		min-height: 0;
	}

	/* ── File sections ── */
	.file-section {
		border-bottom: 1px solid var(--border);
	}

	.section-header {
		display: flex;
		align-items: center;
		padding: 0 12px;
		height: 32px;
		background: var(--bg-secondary);
	}

	.collapse-btn {
		flex: 1;
		display: flex;
		align-items: center;
		gap: 6px;
		background: none;
		border: none;
		color: var(--text-secondary);
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0.07em;
		cursor: pointer;
		text-align: left;
		padding: 0;
	}
	.collapse-btn:hover {
		color: var(--text-primary);
	}

	.chevron {
		font-size: 9px;
		opacity: 0.7;
	}

	.count-badge {
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 8px;
		padding: 0px 6px;
		font-size: 10px;
		color: var(--text-muted);
	}

	.inline-action {
		font-size: 10px;
		padding: 2px 8px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 3px;
		color: var(--text-muted);
		cursor: pointer;
	}
	.inline-action:hover:not(:disabled) {
		color: var(--text-primary);
	}
	.inline-action:disabled {
		opacity: 0.4;
	}

	/* ── File rows ── */
	.file-list {
		padding: 2px 0;
	}

	.file-row {
		display: flex;
		align-items: center;
		gap: 6px;
		height: 28px;
		padding: 0 12px;
		cursor: pointer;
		transition: background 0.08s;
		position: relative;
	}
	.file-row:hover {
		background: var(--bg-tertiary);
	}

	.status-icon {
		width: 16px;
		text-align: center;
		font-size: 11px;
		font-weight: 700;
		flex-shrink: 0;
	}
	.icon-mod {
		color: var(--accent-orange);
	}
	.icon-add {
		color: var(--accent-green);
	}
	.icon-del {
		color: var(--accent-red);
	}
	.icon-conflict {
		color: var(--accent-orange);
	}

	.file-name {
		font-size: 12px;
		font-weight: 600;
		color: var(--text-primary);
		white-space: nowrap;
		flex-shrink: 0;
	}

	.file-dir {
		font-size: 11px;
		color: var(--text-muted);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		flex: 1;
		min-width: 0;
	}

	.stage-btn {
		margin-left: auto;
		width: 20px;
		height: 20px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--bg-primary);
		border: 1px solid var(--border);
		border-radius: 3px;
		font-size: 14px;
		font-weight: bold;
		line-height: 1;
		cursor: pointer;
		opacity: 0;
		flex-shrink: 0;
		transition:
			opacity 0.08s,
			background 0.08s;
	}
	.file-row:hover .stage-btn {
		opacity: 1;
	}
	.stage-btn.plus {
		color: var(--accent-green);
	}
	.stage-btn.plus:hover {
		background: rgba(63, 185, 80, 0.15);
		border-color: var(--accent-green);
	}
	.stage-btn.minus {
		color: var(--accent-orange);
	}
	.stage-btn.minus:hover {
		background: rgba(210, 153, 34, 0.15);
		border-color: var(--accent-orange);
	}
	.stage-btn:disabled {
		opacity: 0.3 !important;
		cursor: not-allowed;
	}

	.empty-files {
		padding: 8px 12px;
		font-size: 11px;
		color: var(--text-muted);
	}

	/* ── Commit box ── */
	.commit-box {
		flex-shrink: 0;
		border-top: 1px solid var(--border);
		padding: 10px 12px;
		background: var(--bg-secondary);
	}

	.commit-tabs {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-bottom: 8px;
	}

	.tab-btn {
		padding: 3px 10px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 4px;
		color: var(--text-secondary);
		font-size: 12px;
		cursor: pointer;
	}
	.tab-btn.active {
		color: var(--text-primary);
		background: #2d333b;
	}

	.amend-label {
		display: flex;
		align-items: center;
		gap: 5px;
		font-size: 12px;
		color: var(--text-muted);
		cursor: pointer;
	}

	.textarea-wrap {
		position: relative;
		margin-bottom: 6px;
	}

	.commit-textarea {
		width: 100%;
		padding: 8px;
		background: var(--bg-primary);
		border: 1px solid var(--border);
		border-radius: 4px;
		color: var(--text-primary);
		font-size: 13px;
		font-family: inherit;
		resize: vertical;
		min-height: 52px;
	}
	.commit-textarea::placeholder {
		color: var(--text-muted);
	}
	.commit-textarea:focus {
		outline: none;
		border-color: var(--text-muted);
	}

	.desc-textarea {
		margin-bottom: 8px;
		min-height: 40px;
		font-size: 12px;
	}

	.char-counter {
		position: absolute;
		top: 6px;
		right: 8px;
		font-size: 10px;
		color: var(--text-muted);
		pointer-events: none;
	}
	.char-counter.over {
		color: var(--accent-red);
	}

	.commit-footer {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.commit-opts-btn {
		font-size: 11px;
		background: none;
		border: none;
		color: var(--text-muted);
		cursor: pointer;
		padding: 0;
	}
	.commit-opts-btn:hover {
		color: var(--text-secondary);
	}

	.commit-btn {
		margin-left: auto;
		padding: 6px 14px;
		font-size: 12px;
		font-weight: 600;
		border-radius: 4px;
		cursor: pointer;
		border: 1px solid var(--accent-green);
		background: rgba(63, 185, 80, 0.15);
		color: var(--accent-green);
		white-space: nowrap;
		transition: background 0.1s;
	}
	.commit-btn:hover:not(:disabled) {
		background: rgba(63, 185, 80, 0.25);
	}
	.commit-btn:disabled {
		border-color: var(--border);
		background: transparent;
		color: var(--text-muted);
		cursor: not-allowed;
		font-weight: 400;
	}

	/* ── Commit mode ── */
	.commit-mode-header {
		flex-direction: column;
		align-items: flex-start;
		height: auto;
		padding: 10px 12px 8px;
		gap: 4px;
	}

	.commit-hash-label {
		font-size: 12px;
		font-family: "JetBrains Mono", monospace;
		color: var(--text-secondary);
		font-weight: 600;
	}

	.explain-btn {
		font-size: 11px;
		padding: 2px 10px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 4px;
		color: var(--text-secondary);
		cursor: pointer;
	}
	.explain-btn:hover {
		color: var(--text-primary);
	}

	.author-section {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 12px;
		border-bottom: 1px solid var(--border);
	}

	.author-avatar {
		width: 32px;
		height: 32px;
		border-radius: 50%;
		background: linear-gradient(135deg, #3fb950, #8b949e);
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 12px;
		font-weight: 700;
		color: #0d1117;
		flex-shrink: 0;
	}

	.author-details {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.author-name {
		font-size: 13px;
		font-weight: 600;
		color: var(--text-primary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.author-meta {
		font-size: 11px;
		color: var(--text-muted);
	}

	.commit-message-display {
		padding: 10px 12px;
		font-size: 13px;
		color: var(--text-primary);
		border-bottom: 1px solid var(--border);
		word-break: break-word;
	}

	.commit-summary-line {
		padding: 6px 12px;
		font-size: 11px;
		color: var(--text-muted);
		border-bottom: 1px solid var(--border);
	}

	.commit-file-list {
		padding: 4px 0;
	}

	.commit-file-row {
		padding: 0 8px;
	}

	.loading-files {
		padding: 8px 12px;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.sk-file-row {
		height: 22px;
		border-radius: 3px;
		background: linear-gradient(
			90deg,
			var(--bg-secondary) 0%,
			var(--bg-tertiary) 50%,
			var(--bg-secondary) 100%
		);
		background-size: 200% 100%;
		animation: shimmer 1.2s ease-in-out infinite;
	}
	@keyframes shimmer {
		0% {
			background-position: 200% 0;
		}
		100% {
			background-position: -200% 0;
		}
	}
</style>
