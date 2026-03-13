<script lang="ts">
	import { open } from '@tauri-apps/plugin-dialog';
	import { onMount } from 'svelte';
	import { loadRepo } from '$lib/store';
	import { getRecentRepositories } from '$lib/tauri';
	import type { RepoRecord } from '$lib/types';

	let recentRepos = $state<RepoRecord[]>([]);
	let toastMessage = $state<string | null>(null);
	let cloneUrl = $state('');
	let isTauri = $state(false);

	onMount(() => {
		isTauri = typeof window !== 'undefined' && !!window.__TAURI__;
		loadRecentRepos();
	});

	async function loadRecentRepos() {
		if (!isTauri) return;
		try {
			recentRepos = await getRecentRepositories(10);
		} catch {
			recentRepos = [];
		}
	}

	async function openFolder() {
		if (!isTauri) {
			toastMessage = 'Please run GitFast from the desktop app (cargo tauri dev)';
			setTimeout(() => (toastMessage = null), 4000);
			return;
		}
		try {
			const selected = await open({
				directory: true,
				multiple: false
			});
			if (selected && typeof selected === 'string') {
				await loadRepo(selected);
			}
		} catch (err) {
			toastMessage = err instanceof Error ? err.message : String(err);
			setTimeout(() => (toastMessage = null), 4000);
		}
	}

	function showComingSoon() {
		toastMessage = 'Coming soon';
		setTimeout(() => (toastMessage = null), 2500);
	}

	function openRepo(repo: RepoRecord) {
		loadRepo(repo.path);
	}

	function formatDate(timestamp: number) {
		const d = new Date(timestamp * 1000);
		const now = new Date();
		const diff = now.getTime() - d.getTime();
		if (diff < 86400000) return d.toLocaleTimeString();
		if (diff < 604800000) return d.toLocaleDateString();
		return d.toLocaleDateString();
	}
</script>

<div class="welcome">
	<div class="welcome-grid">
		<!-- LEFT COLUMN -->
		<div class="left-col">
			<h1 class="logo">⚡ GitFast</h1>
			<p class="subtitle">A fast Git client</p>

			<button class="btn-primary" onclick={openFolder}>Open Repository</button>

			<div class="clone-section">
				<input
					type="text"
					bind:value={cloneUrl}
					placeholder="Repository URL"
					class="clone-input"
					disabled
				/>
				<button class="btn-secondary" onclick={showComingSoon}>
					Clone
				</button>
			</div>
		</div>

		<!-- RIGHT COLUMN -->
		<div class="right-col">
			<h2 class="section-title">Recent Repositories</h2>
			{#if recentRepos.length === 0}
				<p class="empty-state">No recent repositories</p>
			{:else}
				<div class="repo-list">
					{#each recentRepos as repo (repo.id)}
						<button
							type="button"
							class="repo-row"
							onclick={() => openRepo(repo)}
						>
							<span class="repo-name">{repo.name}</span>
							<span class="repo-path">{repo.path}</span>
							<span class="repo-date">{formatDate(repo.last_opened)}</span>
						</button>
					{/each}
				</div>
			{/if}
		</div>
	</div>

	{#if toastMessage}
		<div class="toast">{toastMessage}</div>
	{/if}
</div>

<style>
	.welcome {
		min-height: 100vh;
		background: #0d1117;
		display: flex;
		align-items: center;
		justify-content: center;
		position: relative;
	}

	.welcome-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 64px;
		max-width: 900px;
		width: 100%;
		padding: 48px;
	}

	.left-col {
		display: flex;
		flex-direction: column;
		gap: 24px;
	}

	.logo {
		font-size: 48px;
		font-weight: 700;
		color: white;
		letter-spacing: -1px;
		margin: 0;
	}

	.subtitle {
		color: var(--text-secondary);
		font-size: 18px;
		margin: 0 0 8px 0;
	}

	.btn-primary {
		background: var(--accent-green);
		color: white;
		border: none;
		padding: 12px 24px;
		border-radius: 6px;
		font-size: 14px;
		font-weight: 600;
		cursor: pointer;
		width: fit-content;
	}
	.btn-primary:hover {
		background: #46c35b;
	}

	.clone-section {
		display: flex;
		gap: 8px;
		margin-top: 8px;
	}

	.clone-input {
		flex: 1;
		background: var(--bg-secondary);
		border: 1px solid var(--border);
		border-radius: 6px;
		padding: 10px 14px;
		color: var(--text-primary);
		font-size: 14px;
	}
	.clone-input::placeholder {
		color: var(--text-muted);
	}
	.clone-input:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.btn-secondary {
		background: var(--bg-tertiary);
		color: var(--text-primary);
		border: 1px solid var(--border);
		padding: 10px 20px;
		border-radius: 6px;
		font-size: 14px;
		cursor: pointer;
	}
	.btn-secondary:hover {
		background: var(--bg-secondary);
	}

	.right-col {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.section-title {
		font-size: 18px;
		font-weight: 600;
		color: var(--text-primary);
		margin: 0;
	}

	.empty-state {
		color: var(--text-muted);
		font-size: 14px;
		margin: 0;
	}

	.repo-list {
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
		max-height: 400px;
		overflow-y: auto;
	}

	.repo-row {
		padding: 12px 16px;
		background: var(--bg-secondary);
		border: 1px solid var(--border);
		border-radius: 6px;
		cursor: pointer;
		display: flex;
		flex-direction: column;
		gap: 4px;
		transition: background 0.15s;
		text-align: left;
		color: inherit;
		font: inherit;
	}
	.repo-row:hover {
		background: var(--bg-tertiary);
	}

	.repo-name {
		font-weight: 600;
		color: var(--text-primary);
	}

	.repo-path {
		font-size: 12px;
		color: var(--text-secondary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.repo-date {
		font-size: 12px;
		color: var(--text-muted);
	}

	.toast {
		position: fixed;
		bottom: 24px;
		left: 50%;
		transform: translateX(-50%);
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		padding: 12px 24px;
		border-radius: 6px;
		color: var(--text-primary);
		font-size: 14px;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
		animation: fadeIn 0.2s ease;
	}
	@keyframes fadeIn {
		from {
			opacity: 0;
			transform: translateX(-50%) translateY(8px);
		}
		to {
			opacity: 1;
			transform: translateX(-50%) translateY(0);
		}
	}
</style>
