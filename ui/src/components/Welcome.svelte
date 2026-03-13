<script lang="ts">
	import { open } from "@tauri-apps/plugin-dialog";
	import { onMount } from "svelte";
	import { loadRepo } from "$lib/store";
	import { getRecentRepositories } from "$lib/tauri";
	import type { RepoRecord } from "$lib/types";

	let recentRepos = $state<RepoRecord[]>([]);
	let toastMessage = $state<string | null>(null);
	let cloneUrl = $state("");
	let isTauri = $state(false);

	onMount(() => {
		isTauri = typeof window !== "undefined" && !!(window as Window & { __TAURI__?: unknown }).__TAURI__;
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
			toastMessage =
				"Please run GitAxon from the desktop app (cargo tauri dev)";
			setTimeout(() => (toastMessage = null), 4000);
			return;
		}
		try {
			const selected = await open({
				directory: true,
				multiple: false,
			});
			if (selected && typeof selected === "string") {
				await loadRepo(selected);
			}
		} catch (err) {
			toastMessage = err instanceof Error ? err.message : String(err);
			setTimeout(() => (toastMessage = null), 4000);
		}
	}

	function showComingSoon() {
		toastMessage = "Coming soon";
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

	function getRepoTag(timestamp: number): string {
		const diff = Date.now() - timestamp * 1000;
		if (diff < 86400000) return "Active today";
		if (diff < 604800000) return "This week";
		return "Older";
	}

	function truncatePath(path: string, maxLen = 45): string {
		if (path.length <= maxLen) return path;
		return path.slice(0, maxLen) + "…";
	}
</script>

<div class="app">
	<!-- Top bar -->
	<div class="topbar">
		<div class="breadcrumb">
			<span>Workspace</span>
			<span class="sep">/</span>
			<span class="active">Home</span>
		</div>
		<div class="topbar-actions">
			<button class="topbar-btn">Local-first desktop client</button>
			<button class="topbar-btn active">Today</button>
		</div>
	</div>

	<div class="body">
		<!-- Sidebar -->
		<div class="sidebar">
			<div class="sidebar-logo">
				<div class="logo-text">GitAxon</div>
				<div class="logo-sub">Linear-inspired workspace</div>
			</div>

			<div class="sidebar-section">
				<div class="sidebar-section-label">Workspace</div>
				<div class="sidebar-item active">
					<svg
						width="15"
						height="15"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
						><path
							d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"
						/><polyline points="9 22 9 12 15 12 15 22" /></svg
					>
					Home
				</div>
				<div class="sidebar-item">
					<svg
						width="15"
						height="15"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
						><circle cx="12" cy="12" r="3" /><line
							x1="3"
							y1="12"
							x2="9"
							y2="12"
						/><line x1="15" y1="12" x2="21" y2="12" /><line
							x1="12"
							y1="3"
							x2="12"
							y2="9"
						/><line x1="12" y1="15" x2="12" y2="21" /></svg
					>
					Repositories
				</div>
				<div class="sidebar-item">
					<svg
						width="15"
						height="15"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
						><polyline
							points="22 12 18 12 15 21 9 3 6 12 2 12"
						/></svg
					>
					Recent activity
				</div>
			</div>

			{#if recentRepos.length > 0}
				<div class="sidebar-section">
					<div class="sidebar-section-label">Pinned</div>
					{#each recentRepos.slice(0, 3) as repo (repo.id)}
						<button
							type="button"
							class="sidebar-item"
							onclick={() => openRepo(repo)}
						>
							<svg
								width="15"
								height="15"
								viewBox="0 0 24 24"
								fill="none"
								stroke="currentColor"
								stroke-width="2"
								stroke-linecap="round"
								stroke-linejoin="round"
								><path
									d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"
								/></svg
							>
							{repo.name}
						</button>
					{/each}
				</div>
			{/if}

			<div class="sidebar-bottom">
				<div class="import-box-title">Import a repository</div>
				<div class="import-box-desc">
					Open a local folder or paste a remote URL to get started.
				</div>
				<button class="btn-new-import" onclick={openFolder}
					>New import</button
				>
			</div>
		</div>

		<!-- Main -->
		<div class="main">
			<!-- Hero -->
			<div class="hero">
				<div class="hero-left">
					<div class="hero-label">Desktop Git Workspace</div>
					<div class="hero-title">GitAxon</div>
					<div class="hero-desc">
						A denser, more premium black workspace with a cleaner
						wordmark and one standout command panel for everyday Git
						flow.
					</div>
					<div class="hero-actions">
						<button class="btn-open-repo" onclick={openFolder}>
							<svg
								width="14"
								height="14"
								viewBox="0 0 24 24"
								fill="none"
								stroke="currentColor"
								stroke-width="2"
								stroke-linecap="round"
								stroke-linejoin="round"
								><path
									d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"
								/></svg
							>
							Open repository
						</button>
						<button
							class="btn-new-workspace"
							onclick={showComingSoon}
						>
							<svg
								width="13"
								height="13"
								viewBox="0 0 24 24"
								fill="none"
								stroke="currentColor"
								stroke-width="2"
								stroke-linecap="round"
								stroke-linejoin="round"
								><line x1="12" y1="5" x2="12" y2="19" /><line
									x1="5"
									y1="12"
									x2="19"
									y2="12"
								/></svg
							>
							New workspace
						</button>
					</div>
				</div>

				<div class="hero-right">
					<div class="standout-label">Standout panel</div>
					<div class="quick-clone-card">
						<div class="qc-header">
							<span class="qc-title">Quick clone</span>
							<span class="qc-kbd">⌘ K</span>
						</div>
						<div class="qc-desc">
							Paste a repository URL and jump directly into a
							focused workspace.
						</div>
						<div class="qc-actions">
							<button class="btn-paste" onclick={showComingSoon}>
								<svg
									width="12"
									height="12"
									viewBox="0 0 24 24"
									fill="none"
									stroke="currentColor"
									stroke-width="2"
									stroke-linecap="round"
									stroke-linejoin="round"
									><path
										d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"
									/><path
										d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"
									/></svg
								>
								Paste repo
							</button>
							<button class="btn-clone" onclick={showComingSoon}
								>Clone</button
							>
						</div>
					</div>
					<div class="standout-footer">
						Minimal on the outside, command-focused on the inside.
					</div>
				</div>
			</div>

			<!-- Repos table -->
			<div class="repos-section">
				<div class="repos-header">
					<div class="repos-title">Recent repositories</div>
					<div class="repos-count">
						{recentRepos.length}
						{recentRepos.length === 1 ? "entry" : "entries"}
					</div>
				</div>

				{#if recentRepos.length === 0}
					<div class="empty-state">
						<div class="empty-icon">
							<svg
								width="32"
								height="32"
								viewBox="0 0 24 24"
								fill="none"
								stroke="currentColor"
								stroke-width="1.5"
								stroke-linecap="round"
								stroke-linejoin="round"
								><circle cx="12" cy="12" r="3" /><line
									x1="3"
									y1="12"
									x2="9"
									y2="12"
								/><line x1="15" y1="12" x2="21" y2="12" /><line
									x1="12"
									y1="3"
									x2="12"
									y2="9"
								/><line x1="12" y1="15" x2="12" y2="21" /></svg
							>
						</div>
						<div class="empty-text">No recent repositories</div>
						<div class="empty-hint">
							Open a local folder to get started
						</div>
					</div>
				{:else}
					<table class="repo-table">
						<thead>
							<tr>
								<th style="width:36%">Name</th>
								<th style="width:42%">Path</th>
								<th>Last opened</th>
							</tr>
						</thead>
						<tbody>
							{#each recentRepos as repo (repo.id)}
								<tr onclick={() => openRepo(repo)}>
									<td>
										<div class="td-name">
											<div class="repo-icon-cell">
												<svg
													width="14"
													height="14"
													viewBox="0 0 24 24"
													fill="none"
													stroke="currentColor"
													stroke-width="2"
													stroke-linecap="round"
													stroke-linejoin="round"
													><circle
														cx="12"
														cy="12"
														r="3"
													/><line
														x1="3"
														y1="12"
														x2="9"
														y2="12"
													/><line
														x1="15"
														y1="12"
														x2="21"
														y2="12"
													/><line
														x1="12"
														y1="3"
														x2="12"
														y2="9"
													/><line
														x1="12"
														y1="15"
														x2="12"
														y2="21"
													/></svg
												>
											</div>
											<div>
												<div class="repo-name-text">
													{repo.name}
												</div>
												<div class="repo-tag">
													{getRepoTag(
														repo.last_opened,
													)}
												</div>
											</div>
										</div>
									</td>
									<td
										><span class="repo-path-text"
											>{truncatePath(repo.path)}</span
										></td
									>
									<td
										><span class="repo-time-text"
											>{formatDate(
												repo.last_opened,
											)}</span
										></td
									>
								</tr>
							{/each}
						</tbody>
					</table>
				{/if}
			</div>
		</div>

		<!-- Right panel -->
		<div class="right-panel">
			<div class="rp-section">
				<div class="rp-label">Overview</div>
				<div class="rp-heading">
					A sharper, more unique layout with one command-focused
					surface.
				</div>
				<div class="rp-desc">
					This version keeps the clean wordmark, but shifts the screen
					toward a tighter, premium, Linear-like structure with darker
					layering.
				</div>
			</div>

			<div class="rp-section">
				<div class="rp-label">Session</div>
				<div class="rp-row">
					<span class="rp-row-label">Recent repos</span>
					<span class="rp-row-value">{recentRepos.length}</span>
				</div>
				<div class="rp-row">
					<span class="rp-row-label">Primary action</span>
					<span class="rp-row-value">Open repository</span>
				</div>
				<div class="rp-row">
					<span class="rp-row-label">Look</span>
					<span class="rp-row-value">Black premium</span>
				</div>
			</div>

			<div class="rp-section">
				<div class="rp-label">Flow</div>
				<div class="rp-flow-item">
					<div class="rp-flow-dot"></div>
					<div>
						<div class="rp-flow-title">Open local project</div>
						<div class="rp-flow-desc">
							Jump directly into a repository from the hero
							actions.
						</div>
					</div>
				</div>
				<div class="rp-flow-item">
					<div class="rp-flow-dot"></div>
					<div>
						<div class="rp-flow-title">Clone remote</div>
						<div class="rp-flow-desc">
							Use the standout panel to paste a URL and create a
							new workspace fast.
						</div>
					</div>
				</div>
				<div class="rp-flow-item">
					<div class="rp-flow-dot"></div>
					<div>
						<div class="rp-flow-title">Resume work</div>
						<div class="rp-flow-desc">
							Continue from recent repositories with more visual
							separation between rows.
						</div>
					</div>
				</div>
			</div>
		</div>
	</div>

	{#if toastMessage}
		<div class="toast">{toastMessage}</div>
	{/if}
</div>

<style>
	/* ── Design tokens ── */
	:root {
		--ga-bg: #0a0a0a;
		--ga-sidebar-bg: #0f0f0f;
		--ga-surface: #161616;
		--ga-surface2: #1c1c1c;
		--ga-surface3: #222222;
		--ga-border: #2a2a2a;
		--ga-border2: #333333;
		--ga-text: #ffffff;
		--ga-text-muted: #888888;
		--ga-text-dim: #555555;
	}

	/* ── App shell ── */
	.app {
		display: flex;
		flex-direction: column;
		height: 100vh;
		background: var(--ga-bg);
		border: 1px solid var(--ga-border);
		border-radius: 10px;
		overflow: hidden;
		font-family: "Space Grotesk", sans-serif;
		font-size: 14px;
		line-height: 1.5;
		color: var(--ga-text);
	}

	/* ── Top bar ── */
	.topbar {
		display: flex;
		align-items: center;
		padding: 0 24px;
		height: 48px;
		border-bottom: 1px solid var(--ga-border);
		background: var(--ga-bg);
		flex-shrink: 0;
	}

	.breadcrumb {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 13px;
		color: var(--ga-text-muted);
		margin-left: 280px;
	}
	.breadcrumb span.active {
		color: var(--ga-text);
		font-weight: 500;
	}
	.breadcrumb .sep {
		color: var(--ga-text-dim);
	}

	.topbar-actions {
		margin-left: auto;
		display: flex;
		gap: 8px;
	}

	.topbar-btn {
		background: var(--ga-surface2);
		border: 1px solid var(--ga-border2);
		color: var(--ga-text);
		font-family: "Space Grotesk", sans-serif;
		font-size: 12px;
		font-weight: 500;
		padding: 5px 14px;
		border-radius: 6px;
		cursor: pointer;
		transition: background 0.15s;
	}
	.topbar-btn:hover {
		background: var(--ga-surface3);
	}
	.topbar-btn.active {
		background: var(--ga-surface3);
		border-color: #444;
	}

	/* ── Body layout ── */
	.body {
		display: flex;
		flex: 1;
		overflow: hidden;
	}

	/* ── Sidebar ── */
	.sidebar {
		width: 300px;
		flex-shrink: 0;
		background: var(--ga-sidebar-bg);
		border-right: 1px solid var(--ga-border);
		display: flex;
		flex-direction: column;
		overflow-y: auto;
	}

	.sidebar-logo {
		padding: 20px 20px 6px;
	}
	.sidebar-logo .logo-text {
		font-size: 20px;
		font-weight: 700;
		letter-spacing: -0.03em;
		color: #fff;
	}
	.sidebar-logo .logo-sub {
		font-size: 11px;
		color: var(--ga-text-dim);
		margin-top: 2px;
	}

	.sidebar-section {
		padding: 20px 12px 4px;
	}
	.sidebar-section-label {
		font-size: 11px;
		color: var(--ga-text-dim);
		font-weight: 500;
		letter-spacing: 0.04em;
		padding: 0 8px;
		margin-bottom: 4px;
	}

	.sidebar-item {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 7px 10px;
		border-radius: 6px;
		font-size: 13px;
		font-weight: 500;
		color: var(--ga-text-muted);
		cursor: pointer;
		transition:
			background 0.12s,
			color 0.12s;
		background: none;
		border: none;
		width: 100%;
		text-align: left;
		font-family: "Space Grotesk", sans-serif;
	}
	.sidebar-item:hover {
		background: var(--ga-surface);
		color: #fff;
	}
	.sidebar-item.active {
		background: var(--ga-surface2);
		color: #fff;
	}
	.sidebar-item :global(svg) {
		flex-shrink: 0;
		opacity: 0.7;
	}
	.sidebar-item.active :global(svg) {
		opacity: 1;
	}

	.sidebar-bottom {
		margin-top: auto;
		padding: 16px;
		border-top: 1px solid var(--ga-border);
	}
	.import-box-title {
		font-size: 13px;
		font-weight: 600;
		color: #fff;
		margin-bottom: 4px;
	}
	.import-box-desc {
		font-size: 11.5px;
		color: var(--ga-text-muted);
		line-height: 1.5;
		margin-bottom: 12px;
	}
	.btn-new-import {
		background: transparent;
		border: 1px solid var(--ga-border2);
		color: #fff;
		font-family: "Space Grotesk", sans-serif;
		font-size: 12px;
		font-weight: 500;
		padding: 7px 16px;
		border-radius: 6px;
		cursor: pointer;
		transition: background 0.15s;
	}
	.btn-new-import:hover {
		background: var(--ga-surface2);
	}

	/* ── Main content ── */
	.main {
		flex: 1;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
	}

	/* Hero section */
	.hero {
		display: grid;
		grid-template-columns: 1fr 480px;
		gap: 0;
		border-bottom: 1px solid var(--ga-border);
	}

	.hero-left {
		padding: 36px;
		border-right: 1px solid var(--ga-border);
	}
	.hero-label {
		font-size: 10.5px;
		font-weight: 500;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--ga-text-dim);
		margin-bottom: 14px;
	}
	.hero-title {
		font-size: 52px;
		font-weight: 700;
		letter-spacing: -0.04em;
		line-height: 1;
		color: #fff;
		margin-bottom: 20px;
	}
	.hero-desc {
		font-size: 16px;
		font-weight: 400;
		color: var(--ga-text-muted);
		line-height: 1.6;
		max-width: 380px;
		margin-bottom: 32px;
	}
	.hero-actions {
		display: flex;
		flex-direction: column;
		gap: 8px;
		width: fit-content;
	}

	.btn-open-repo {
		display: flex;
		align-items: center;
		gap: 10px;
		background: transparent;
		border: 1px solid var(--ga-border2);
		color: #fff;
		font-family: "Space Grotesk", sans-serif;
		font-size: 13px;
		font-weight: 500;
		padding: 10px 20px;
		border-radius: 8px;
		cursor: pointer;
		transition: background 0.15s;
	}
	.btn-open-repo:hover {
		background: var(--ga-surface2);
	}

	.btn-new-workspace {
		display: flex;
		align-items: center;
		gap: 8px;
		background: transparent;
		border: none;
		color: var(--ga-text-muted);
		font-family: "Space Grotesk", sans-serif;
		font-size: 13px;
		font-weight: 500;
		padding: 10px 20px;
		border-radius: 8px;
		cursor: pointer;
		transition:
			color 0.15s,
			background 0.15s;
	}
	.btn-new-workspace:hover {
		color: #fff;
		background: var(--ga-surface);
	}

	/* Standout panel */
	.hero-right {
		padding: 24px;
		display: flex;
		flex-direction: column;
	}
	.standout-label {
		font-size: 11px;
		color: var(--ga-text-dim);
		margin-bottom: 16px;
	}
	.quick-clone-card {
		background: var(--ga-surface);
		border: 1px solid var(--ga-border2);
		border-radius: 10px;
		padding: 18px;
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.qc-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}
	.qc-title {
		font-size: 13px;
		font-weight: 600;
		color: #fff;
	}
	.qc-kbd {
		background: var(--ga-surface2);
		border: 1px solid var(--ga-border2);
		border-radius: 4px;
		padding: 2px 7px;
		font-size: 11px;
		color: var(--ga-text-muted);
		font-family: "Space Grotesk", monospace;
	}
	.qc-desc {
		font-size: 12px;
		color: var(--ga-text-muted);
		line-height: 1.5;
	}
	.qc-actions {
		display: flex;
		gap: 8px;
		margin-top: 4px;
	}
	.btn-paste {
		display: flex;
		align-items: center;
		gap: 7px;
		flex: 1;
		background: var(--ga-surface2);
		border: 1px solid var(--ga-border2);
		color: var(--ga-text-muted);
		font-family: "Space Grotesk", sans-serif;
		font-size: 12px;
		font-weight: 500;
		padding: 8px 14px;
		border-radius: 6px;
		cursor: pointer;
		transition: background 0.15s;
	}
	.btn-paste:hover {
		background: var(--ga-surface3);
		color: #fff;
	}

	.btn-clone {
		background: #fff;
		border: 1px solid #fff;
		color: #000;
		font-family: "Space Grotesk", sans-serif;
		font-size: 12px;
		font-weight: 600;
		padding: 8px 18px;
		border-radius: 6px;
		cursor: pointer;
		transition: background 0.15s;
	}
	.btn-clone:hover {
		background: #e0e0e0;
	}

	.standout-footer {
		font-size: 11.5px;
		color: var(--ga-text-dim);
		margin-top: 14px;
		line-height: 1.5;
	}

	/* ── Repo table ── */
	.repos-section {
		padding: 28px 36px;
		flex: 1;
	}
	.repos-header {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		margin-bottom: 18px;
	}
	.repos-title {
		font-size: 22px;
		font-weight: 700;
		letter-spacing: -0.02em;
		color: #fff;
	}
	.repos-count {
		font-size: 12px;
		color: var(--ga-text-dim);
	}

	/* Empty state */
	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 48px 0;
		gap: 12px;
	}
	.empty-icon {
		color: var(--ga-text-dim);
		opacity: 0.5;
		margin-bottom: 4px;
	}
	.empty-text {
		font-size: 15px;
		font-weight: 600;
		color: var(--ga-text-muted);
	}
	.empty-hint {
		font-size: 12px;
		color: var(--ga-text-dim);
	}

	.repo-table {
		width: 100%;
		border-collapse: separate;
		border-spacing: 0 2px;
	}
	.repo-table thead tr {
		border-bottom: 1px solid var(--ga-border);
	}
	.repo-table th {
		text-align: left;
		font-size: 11.5px;
		font-weight: 500;
		color: var(--ga-text-dim);
		padding: 0 0 10px;
	}
	.repo-table th:last-child {
		text-align: right;
	}

	.repo-table tbody tr {
		cursor: pointer;
		transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
		border-left: 2px solid transparent;
		border-radius: 8px;
		position: relative;
	}
	.repo-table tbody tr:hover {
		background: var(--ga-surface);
		border-left: 2px solid #fff;
		box-shadow:
			0 2px 16px rgba(255, 255, 255, 0.03),
			inset 0 0 0 1px rgba(255, 255, 255, 0.04);
		transform: translateX(2px);
	}
	.repo-table tbody tr:active {
		transform: translateX(2px) scale(0.995);
	}

	.repo-table td {
		padding: 14px 12px;
		vertical-align: middle;
		transition: color 0.2s ease;
	}
	.repo-table td:first-child {
		border-radius: 8px 0 0 8px;
	}
	.repo-table td:last-child {
		text-align: right;
		border-radius: 0 8px 8px 0;
	}

	.td-name {
		display: flex;
		align-items: center;
		gap: 14px;
	}
	.repo-icon-cell {
		width: 36px;
		height: 36px;
		background: var(--ga-surface2);
		border: 1px solid var(--ga-border2);
		border-radius: 7px;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		color: var(--ga-text-dim);
		transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
	}
	.repo-table tbody tr:hover .repo-icon-cell {
		background: var(--ga-surface3);
		border-color: #444;
		color: #fff;
	}
	.repo-name-text {
		font-size: 14px;
		font-weight: 600;
		color: #fff;
		transition: color 0.15s ease;
	}
	.repo-tag {
		font-size: 11px;
		color: var(--ga-text-dim);
		margin-top: 1px;
		transition: color 0.15s ease;
	}
	.repo-table tbody tr:hover .repo-tag {
		color: var(--ga-text-muted);
	}
	.repo-path-text {
		font-size: 12px;
		color: var(--ga-text-dim);
		font-family: "Space Grotesk", monospace;
		transition: color 0.15s ease;
	}
	.repo-table tbody tr:hover .repo-path-text {
		color: var(--ga-text-muted);
	}
	.repo-time-text {
		font-size: 12px;
		color: var(--ga-text-muted);
		transition: color 0.15s ease;
	}
	.repo-table tbody tr:hover .repo-time-text {
		color: #ccc;
	}

	/* ── Right panel ── */
	.right-panel {
		width: 300px;
		flex-shrink: 0;
		border-left: 1px solid var(--ga-border);
		overflow-y: auto;
		display: flex;
		flex-direction: column;
	}
	.rp-section {
		padding: 22px;
		border-bottom: 1px solid var(--ga-border);
	}
	.rp-label {
		font-size: 10.5px;
		font-weight: 500;
		letter-spacing: 0.1em;
		text-transform: uppercase;
		color: var(--ga-text-dim);
		margin-bottom: 12px;
	}
	.rp-heading {
		font-size: 18px;
		font-weight: 700;
		letter-spacing: -0.02em;
		color: #fff;
		line-height: 1.3;
		margin-bottom: 10px;
	}
	.rp-desc {
		font-size: 12px;
		color: var(--ga-text-muted);
		line-height: 1.6;
	}
	.rp-row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 8px 0;
		border-bottom: 1px solid var(--ga-border);
		font-size: 12.5px;
	}
	.rp-row:last-child {
		border-bottom: none;
	}
	.rp-row-label {
		color: var(--ga-text-muted);
	}
	.rp-row-value {
		color: #fff;
		font-weight: 600;
	}

	.rp-flow-item {
		display: flex;
		align-items: flex-start;
		gap: 10px;
		padding: 6px 0;
	}
	.rp-flow-dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: #fff;
		margin-top: 5px;
		flex-shrink: 0;
	}
	.rp-flow-title {
		font-size: 13px;
		font-weight: 600;
		color: #fff;
		margin-bottom: 2px;
	}
	.rp-flow-desc {
		font-size: 11.5px;
		color: var(--ga-text-muted);
		line-height: 1.5;
	}

	/* ── Toast ── */
	.toast {
		position: fixed;
		bottom: 24px;
		left: 50%;
		transform: translateX(-50%);
		background: var(--ga-surface3);
		border: 1px solid var(--ga-border2);
		padding: 12px 24px;
		border-radius: 8px;
		color: #fff;
		font-size: 13px;
		font-weight: 500;
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
		animation: toastIn 0.25s cubic-bezier(0.16, 1, 0.3, 1);
		z-index: 999;
	}
	@keyframes toastIn {
		from {
			opacity: 0;
			transform: translateX(-50%) translateY(12px);
		}
		to {
			opacity: 1;
			transform: translateX(-50%) translateY(0);
		}
	}
</style>
