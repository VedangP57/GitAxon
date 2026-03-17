<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import { getCurrentWindow } from "@tauri-apps/api/window";
	import CenterPanel from "./CenterPanel.svelte";
	import RightPanel from "./RightPanel.svelte";
	import BranchSidebar from "./BranchSidebar.svelte";
	import Toast from "./Toast.svelte";
	import {
		loadRepo,
		currentRepo,
		isLoading,
		fetchFromRemote,
		branches,
		goHome,
	} from "$lib/store";
	import {
		pull,
		push,
		stashPush,
		stashPop,
		openTerminalAt,
		getRepoIdentity,
		getSshProfiles,
		switchRepoIdentity,
	} from "$lib/tauri";
	import { showToast } from "$lib/toast";
	import { openCreateBranchForm } from "$lib/store";

	let repoName = $derived(
		$currentRepo
			? ($currentRepo.split(/[/\\]/).filter(Boolean).pop() ??
					"Repository")
			: "Repository",
	);

	const currentBranchName = $derived(
		$branches.find((b) => b.isHead && !b.isRemote)?.name ?? "",
	);

	// ── Resizable panels ─────────────────────────────────────────
	const LEFT_MIN = 160;
	const LEFT_MAX = 400;
	const RIGHT_MIN = 220;
	const RIGHT_MAX = 500;

	function stored(key: string, def: number): number {
		try {
			const v = localStorage.getItem(key);
			if (v) {
				const n = parseInt(v, 10);
				if (!isNaN(n)) return n;
			}
		} catch {}
		return def;
	}
	function store(key: string, val: number) {
		try {
			localStorage.setItem(key, String(val));
		} catch {}
	}

	let leftWidth = $state(stored("gax-left", 180));
	let rightWidth = $state(stored("gax-right", 300));
	let resizing = $state<"left" | "right" | null>(null);

	function startResize(which: "left" | "right") {
		resizing = which;
	}

	function onMouseMove(e: MouseEvent) {
		if (resizing === "left") {
			const w = Math.max(LEFT_MIN, Math.min(LEFT_MAX, e.clientX));
			leftWidth = w;
			store("gax-left", w);
		} else if (resizing === "right") {
			const w = Math.max(
				RIGHT_MIN,
				Math.min(RIGHT_MAX, window.innerWidth - e.clientX),
			);
			rightWidth = w;
			store("gax-right", w);
		}
	}

	function onMouseUp() {
		resizing = null;
	}

	onMount(() => {
		window.addEventListener("mousemove", onMouseMove);
		window.addEventListener("mouseup", onMouseUp);
		if ($currentRepo) loadRepo($currentRepo);
	});

	onDestroy(() => {
		window.removeEventListener("mousemove", onMouseMove);
		window.removeEventListener("mouseup", onMouseUp);
	});
	// ─────────────────────────────────────────────────────────────

	$effect(() => {
		const name = repoName;
		getCurrentWindow().setTitle("GitAxon — " + name);
	});

	async function onFetch() {
		const repo = $currentRepo;
		if (!repo) return;
		try {
			await fetchFromRemote("origin");
			showToast("Fetch completed", "success");
		} catch (e) {
			showToast(e instanceof Error ? e.message : String(e), "error");
		}
	}

	async function onPull() {
		const repo = $currentRepo;
		if (!repo || !currentBranchName) {
			showToast("No branch selected or detached HEAD", "error");
			return;
		}
		try {
			await pull(repo, "origin", currentBranchName);
			await loadRepo(repo);
			showToast("Pull completed", "success");
		} catch (e) {
			showToast(e instanceof Error ? e.message : String(e), "error");
		}
	}

	async function onStash() {
		const repo = $currentRepo;
		if (!repo) return;
		try {
			await stashPush(repo);
			await loadRepo(repo);
			showToast("Changes stashed", "success");
		} catch (e) {
			showToast(e instanceof Error ? e.message : String(e), "error");
		}
	}

	async function onPop() {
		const repo = $currentRepo;
		if (!repo) return;
		try {
			await stashPop(repo);
			await loadRepo(repo);
			showToast("Stash applied", "success");
		} catch (e) {
			showToast(e instanceof Error ? e.message : String(e), "error");
		}
	}

	function onTerminal() {
		const repo = $currentRepo;
		if (!repo) {
			showToast("No repository open", "error");
			return;
		}
		openTerminalAt(repo).catch((e) =>
			showToast(e instanceof Error ? e.message : String(e), "error"),
		);
	}

	function onUndo() {
		showToast("Undo not implemented", "info");
	}

	function onRedo() {
		showToast("Redo not implemented", "info");
	}

	let branchDropdownOpen = $state(false);
	function toggleBranchDropdown() {
		branchDropdownOpen = !branchDropdownOpen;
	}
	function openCreateBranch() {
		openCreateBranchForm.set(true);
		branchDropdownOpen = false;
	}

	// ── Multi-account Git identity ───────────────────────────────────
	interface GitIdentity {
		name: string;
		email: string;
		ssh_host: string;
		ssh_key: string;
		account_label: string;
		remote_url: string;
		is_correct: boolean;
	}
	interface SshProfile {
		host_alias: string;
		hostname: string;
		identity_file: string;
		label: string;
	}

	let identity = $state<GitIdentity | null>(null);
	let profiles = $state<SshProfile[]>([]);
	let showAccountMenu = $state(false);
	let showSwitchModal = $state(false);
	let switchModalProfile = $state<SshProfile | null>(null);
	let switchName = $state("");
	let switchEmail = $state("");
	let showPushWarning = $state(false);
	let pendingPushIdentity = $state<GitIdentity | null>(null);

	$effect(() => {
		const repo = $currentRepo;
		if (repo) {
			getRepoIdentity(repo)
				.then((id) => {
					identity = id;
				})
				.catch(() => (identity = null));
			getSshProfiles()
				.then((p) => (profiles = p))
				.catch(() => (profiles = []));
		} else {
			identity = null;
			profiles = [];
		}
	});

	function openSwitchModal(profile: SshProfile) {
		switchModalProfile = profile;
		switchName = identity?.name ?? "";
		switchEmail = "";
		showSwitchModal = true;
	}

	async function confirmSwitch() {
		const repo = $currentRepo;
		const profile = switchModalProfile;
		if (!repo || !profile) return;
		const name = switchName.trim();
		const email = switchEmail.trim();
		if (!email) {
			showToast("Please enter an email address", "error");
			return;
		}
		try {
			await switchRepoIdentity(repo, profile.host_alias, name, email);
			identity = await getRepoIdentity(repo);
			showAccountMenu = false;
			showSwitchModal = false;
			showToast(`Switched to ${profile.label} account`, "success");
		} catch (e) {
			showToast(e instanceof Error ? e.message : String(e), "error");
		}
	}

	async function executePush() {
		const repo = $currentRepo;
		if (!repo || !currentBranchName) return;
		try {
			await push(repo, "origin", currentBranchName, false);
			await loadRepo(repo);
			showToast("Push completed", "success");
		} catch (e) {
			showToast(e instanceof Error ? e.message : String(e), "error");
		}
	}

	function shouldWarnPush(id: GitIdentity): boolean {
		// Personal key pushing to office repo
		if (id.ssh_key.includes("vedangp57") && id.remote_url.includes("elvee-jewels")) {
			return true;
		}
		// Office key pushing to personal repo (not elvee-jewels, not Sarvadhi-Solutions)
		if (
			id.ssh_key.includes("id_ed25519") &&
			!id.ssh_key.includes("vedangp57") &&
			!id.remote_url.includes("elvee-jewels") &&
			!id.remote_url.includes("Sarvadhi-Solutions")
		) {
			return true;
		}
		return false;
	}

	async function onPush() {
		const repo = $currentRepo;
		if (!repo || !currentBranchName) {
			showToast("No branch selected or detached HEAD", "error");
			return;
		}
		try {
			const id = await getRepoIdentity(repo);
			if (shouldWarnPush(id)) {
				showPushWarning = true;
				pendingPushIdentity = id;
				return;
			}
			await executePush();
		} catch (e) {
			showToast(e instanceof Error ? e.message : String(e), "error");
		}
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="app-shell" class:is-resizing={resizing !== null}>
	<!-- ═══ TOOLBAR ═══ -->
	<header class="toolbar">
		<!-- Left -->
		<div class="tb-left">
			<button class="tb-icon-btn" onclick={goHome} title="Back to Home">
				<svg
					width="16"
					height="16"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
				>
					<path
						d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"
					/><polyline points="9 22 9 12 15 12 15 22" />
				</svg>
			</button>
			<span class="tb-repo-name">{repoName}</span>
			{#if currentBranchName}
				<span class="tb-sep">›</span>
				<span class="tb-branch-name">{currentBranchName}</span>
			{/if}
			<button class="tb-icon-btn" onclick={onFetch} title="Fetch">
				<svg
					width="14"
					height="14"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
				>
					<path d="M23 4v6h-6" /><path d="M1 20v-6h6" /><path
						d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"
					/>
				</svg>
			</button>
		</div>

		<!-- Center action buttons -->
		<div class="tb-center">
			<button class="tb-action-btn" title="Undo" onclick={onUndo}>
				<svg
					width="16"
					height="16"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
					><path d="M3 7v6h6" /><path
						d="M21 17a9 9 0 0 0-9-9 9 9 0 0 0-6 2.3L3 13"
					/></svg
				>
				<span>Undo</span>
			</button>
			<button class="tb-action-btn" title="Redo" onclick={onRedo}>
				<svg
					width="16"
					height="16"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
					><path d="M21 7v6h-6" /><path
						d="M3 17a9 9 0 0 1 9-9 9 9 0 0 1 6 2.3L21 13"
					/></svg
				>
				<span>Redo</span>
			</button>
			<div class="tb-divider"></div>
			<button class="tb-action-btn" onclick={onPull} title="Pull">
				<svg
					width="16"
					height="16"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
					><path d="M12 5v14" /><polyline
						points="19 12 12 19 5 12"
					/></svg
				>
				<span>Pull</span>
			</button>
			<button class="tb-action-btn" onclick={onPush} title="Push">
				<svg
					width="16"
					height="16"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
					><path d="M12 19V5" /><polyline
						points="5 12 12 5 19 12"
					/></svg
				>
				<span>Push</span>
			</button>
			<div class="tb-divider"></div>
			<div class="tb-dropdown-wrap">
				<button
					class="tb-action-btn"
					title="Branch"
					onclick={toggleBranchDropdown}
					aria-expanded={branchDropdownOpen}
					aria-haspopup="true"
				>
					<svg
						width="16"
						height="16"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
						><line x1="6" y1="3" x2="6" y2="15" /><circle
							cx="18"
							cy="6"
							r="3"
						/><circle cx="6" cy="18" r="3" /><path
							d="M18 9a9 9 0 0 1-9 9"
						/></svg
					>
					<span>Branch ▾</span>
				</button>
				{#if branchDropdownOpen}
					<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
					<div
						class="tb-dropdown-backdrop"
						onclick={() => (branchDropdownOpen = false)}
						role="presentation"
					></div>
					<div class="tb-dropdown" role="menu">
						<button
							class="tb-dropdown-item"
							onclick={openCreateBranch}
							role="menuitem">Create branch...</button
						>
						<span class="tb-dropdown-hint"
							>Switch/delete in left sidebar</span
						>
					</div>
				{/if}
			</div>
			<button class="tb-action-btn" title="Stash" onclick={onStash}>
				<svg
					width="16"
					height="16"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
					><rect
						x="2"
						y="7"
						width="20"
						height="14"
						rx="2"
						ry="2"
					/><path
						d="M16 21V5a2 2 0 0 0-2-2h-4a2 2 0 0 0-2 2v16"
					/></svg
				>
				<span>Stash</span>
			</button>
			<button class="tb-action-btn" title="Pop stash" onclick={onPop}>
				<svg
					width="16"
					height="16"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
					><path d="M12 5v14" /><polyline
						points="19 12 12 5 5 12"
					/></svg
				>
				<span>Pop</span>
			</button>
			<div class="tb-divider"></div>
			<button class="tb-action-btn" title="Terminal" onclick={onTerminal}>
				<svg
					width="16"
					height="16"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
					><polyline points="4 17 10 11 4 5" /><line
						x1="12"
						y1="19"
						x2="20"
						y2="19"
					/></svg
				>
				<span>Terminal</span>
			</button>
		</div>

		<!-- Right icons + spinner + account -->
		<div class="tb-right tb-right-wrap">
			{#if $isLoading}
				<div class="spinner" aria-label="Loading"></div>
			{/if}
			<button class="tb-icon-btn" title="Notifications">
				<svg
					width="16"
					height="16"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					><path
						d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"
					/><path d="M13.73 21a2 2 0 0 1-3.46 0" /></svg
				>
			</button>
			<button class="tb-icon-btn" title="Settings">
				<svg
					width="16"
					height="16"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					><circle cx="12" cy="12" r="3" /><path
						d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"
					/></svg
				>
			</button>
			<div class="account-dropdown-wrap">
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<button
					class="account-btn"
					class:account-warning={identity && !identity.is_correct}
					onclick={() => (showAccountMenu = !showAccountMenu)}
					title="Git account: {identity?.account_label ?? '...'}"
				>
					<span class="account-avatar">
						{identity?.name?.[0]?.toUpperCase() ?? '?'}
					</span>
					<span class="account-label">{identity?.account_label ?? '...'}</span>
					<span class="account-email">{identity?.email ?? ''}</span>
				</button>
				{#if showAccountMenu}
					<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
					<div
						class="tb-dropdown-backdrop account-backdrop"
						onclick={() => (showAccountMenu = false)}
					></div>
					<div class="account-menu">
						<div class="account-menu-header">
							<span>Switch Git Account</span>
							<span class="current-repo-label"> for {repoName}</span>
						</div>

						<div class="account-menu-current">
							<span class="menu-label">Current</span>
							<div class="account-row active">
								<div class="account-avatar-lg">
									{identity?.name?.[0]?.toUpperCase() ?? '?'}
								</div>
								<div class="account-info">
									<span class="account-name">{identity?.name ?? '—'}</span>
									<span class="account-email-small">{identity?.email ?? '—'}</span>
									<span class="account-host">{identity?.ssh_host ?? '—'}</span>
								</div>
								<span class="checkmark">✓</span>
							</div>
						</div>

						<div class="account-menu-profiles">
							<span class="menu-label">Switch to</span>
							{#each profiles.filter((p) => p.host_alias !== identity?.ssh_host) as profile}
								<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
								<div
									class="account-row clickable"
									onclick={() => openSwitchModal(profile)}
								>
									<div class="account-avatar-lg">
										{profile.label[0]?.toUpperCase() ?? '?'}
									</div>
									<div class="account-info">
										<span class="account-name">{profile.label}</span>
										<span class="account-host">{profile.host_alias}</span>
										<span class="account-key">{profile.identity_file}</span>
									</div>
								</div>
							{/each}
						</div>
					</div>
				{/if}
			</div>
		</div>
	</header>

	<!-- ═══ MAIN BODY (flex row) ═══ -->
	<div class="main-body">
		<!-- ZONE 1: LEFT SIDEBAR -->
		<aside class="left-panel" style="width: {leftWidth}px;">
			<BranchSidebar />
		</aside>

		<!-- Left resize handle -->
		<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
		<div
			class="resize-handle"
			class:active={resizing === "left"}
			role="separator"
			aria-orientation="vertical"
			onmousedown={() => startResize("left")}
		>
			<span class="resize-dots">⋮</span>
		</div>

		<!-- ZONE 2: CENTER -->
		<main class="center">
			<CenterPanel />
		</main>

		<!-- Right resize handle -->
		<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
		<div
			class="resize-handle"
			class:active={resizing === "right"}
			role="separator"
			aria-orientation="vertical"
			onmousedown={() => startResize("right")}
		>
			<span class="resize-dots">⋮</span>
		</div>

		<!-- ZONE 3: RIGHT PANEL -->
		<aside class="right-panel" style="width: {rightWidth}px;">
			<RightPanel />
		</aside>
	</div>

	<!-- Switch account modal -->
	{#if showSwitchModal}
		<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
		<div
			class="push-warning-overlay"
			onclick={(e) => e.target === e.currentTarget && (showSwitchModal = false)}
		>
			<div class="switch-modal" onclick={(e) => e.stopPropagation()}>
				<h3>Switch to {switchModalProfile?.label ?? ''} account</h3>
				<p class="switch-modal-sub">for {repoName}</p>
				<div class="switch-modal-form">
					<label>Name</label>
					<input
						type="text"
						bind:value={switchName}
						placeholder="Your name"
					/>
					<label>Email</label>
					<input
						type="email"
						bind:value={switchEmail}
						placeholder="your@email.com"
					/>
				</div>
				<p class="switch-modal-note">
					This will set local git config user.name and user.email, and update the
					remote URL to use the correct SSH host.
				</p>
				<div class="switch-modal-actions">
					<button class="btn-secondary" onclick={() => (showSwitchModal = false)}>
						Cancel
					</button>
					<button class="btn-primary" onclick={confirmSwitch}>Switch Account</button>
				</div>
			</div>
		</div>
	{/if}

	<!-- Push warning modal -->
	{#if showPushWarning}
		<div class="push-warning-overlay">
			<div class="push-warning-modal">
				<div class="warning-icon">⚠</div>
				<h3>Wrong Account Warning</h3>
				<p>
					You're about to push to<br />
					<strong>{repoName}</strong><br />
					using <strong>{pendingPushIdentity?.account_label}</strong> account (<strong
						>{pendingPushIdentity?.email}</strong
					>)
				</p>
				<p class="warning-sub">Is this correct?</p>
				<div class="warning-actions">
					<button
						class="btn-danger"
						onclick={() => {
							showPushWarning = false;
							executePush();
						}}
					>
						Push Anyway
					</button>
					<button
						class="btn-secondary"
						onclick={() => {
							showPushWarning = false;
							showAccountMenu = true;
						}}
					>
						Switch Account First
					</button>
					<button
						class="btn-cancel"
						onclick={() => (showPushWarning = false)}
					>
						Cancel
					</button>
				</div>
			</div>
		</div>
	{/if}

	<Toast />
</div>

<style>
	.app-shell {
		display: flex;
		flex-direction: column;
		height: 100vh;
		overflow: hidden;
		background: #0d1117;
	}

	/* Disable text selection while dragging */
	.app-shell.is-resizing {
		user-select: none;
		cursor: col-resize;
	}
	.app-shell.is-resizing * {
		pointer-events: none;
	}
	.app-shell.is-resizing .resize-handle {
		pointer-events: all;
	}

	/* ── Toolbar ── */
	.toolbar {
		height: 48px;
		min-height: 48px;
		background: var(--bg-secondary);
		border-bottom: 1px solid var(--border);
		display: flex;
		align-items: center;
		padding: 0 8px;
		flex-shrink: 0;
		-webkit-app-region: drag;
	}

	.tb-left {
		display: flex;
		align-items: center;
		gap: 6px;
		flex-shrink: 0;
		-webkit-app-region: no-drag;
	}

	.tb-center {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 2px;
		-webkit-app-region: no-drag;
	}

	.tb-right {
		display: flex;
		align-items: center;
		gap: 4px;
		flex-shrink: 0;
		-webkit-app-region: no-drag;
	}

	.tb-repo-name {
		font-size: 13px;
		font-weight: 600;
		color: var(--text-primary);
		white-space: nowrap;
	}

	.tb-sep {
		color: var(--text-muted);
		font-size: 14px;
	}

	.tb-branch-name {
		font-size: 12px;
		color: var(--text-muted);
		white-space: nowrap;
		max-width: 120px;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.tb-icon-btn {
		width: 30px;
		height: 30px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: transparent;
		border: 1px solid transparent;
		border-radius: 4px;
		color: var(--text-muted);
		cursor: pointer;
		transition:
			color 0.1s,
			background 0.1s;
		flex-shrink: 0;
	}
	.tb-icon-btn:hover {
		color: var(--text-primary);
		background: var(--bg-tertiary);
		border-color: var(--border);
	}

	.tb-action-btn {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 2px;
		padding: 0 8px;
		height: 48px;
		min-width: 44px;
		background: transparent;
		border: none;
		border-radius: 0;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 11px;
		transition:
			color 0.1s,
			background 0.1s;
	}
	.tb-action-btn span {
		font-size: 10px;
		white-space: nowrap;
	}
	.tb-action-btn:hover {
		color: var(--text-primary);
		background: var(--bg-tertiary);
	}

	.tb-divider {
		width: 1px;
		height: 28px;
		background: var(--border);
		margin: 0 4px;
		flex-shrink: 0;
	}

	.tb-dropdown-wrap {
		position: relative;
	}
	.tb-dropdown-backdrop {
		position: fixed;
		inset: 0;
		z-index: 99;
	}
	.tb-dropdown {
		position: absolute;
		top: 100%;
		left: 0;
		margin-top: 2px;
		min-width: 200px;
		padding: 4px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 6px;
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
		z-index: 100;
	}
	.tb-dropdown-item {
		display: block;
		width: 100%;
		padding: 8px 12px;
		background: none;
		border: none;
		color: var(--text-primary);
		font-size: 12px;
		text-align: left;
		cursor: pointer;
		border-radius: 4px;
	}
	.tb-dropdown-item:hover {
		background: var(--bg-secondary);
	}
	.tb-dropdown-hint {
		display: block;
		padding: 6px 12px;
		font-size: 10px;
		color: var(--text-muted);
	}

	.tb-profile-btn {
		width: 28px;
		height: 28px;
		border-radius: 50%;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--text-secondary);
		cursor: pointer;
	}
	.tb-profile-btn:hover {
		border-color: var(--text-muted);
	}

	/* ── Main body: flex row ── */
	.main-body {
		flex: 1;
		display: flex;
		flex-direction: row;
		min-height: 0;
		overflow: hidden;
	}

	/* ── Panels ── */
	.left-panel {
		flex-shrink: 0;
		overflow: hidden;
		min-width: 160px;
		max-width: 400px;
	}

	.center {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		position: relative;
	}

	.right-panel {
		flex-shrink: 0;
		overflow: hidden;
		min-width: 220px;
		max-width: 500px;
	}

	/* ── Resize handles ── */
	.resize-handle {
		flex-shrink: 0;
		width: 1px;
		background: var(--border);
		cursor: col-resize;
		display: flex;
		align-items: center;
		justify-content: center;
		transition:
			background 0.15s,
			width 0.15s;
		position: relative;
		z-index: 10;
	}

	.resize-handle:hover,
	.resize-handle.active {
		background: var(--text-muted);
		width: 3px;
	}

	.resize-dots {
		color: var(--text-muted);
		font-size: 11px;
		line-height: 1;
		opacity: 0;
		transition: opacity 0.15s;
		pointer-events: none;
		writing-mode: vertical-lr;
		letter-spacing: -3px;
	}
	.resize-handle:hover .resize-dots,
	.resize-handle.active .resize-dots {
		opacity: 1;
	}

	/* Spinner */
	.spinner {
		width: 16px;
		height: 16px;
		border: 2px solid var(--border);
		border-top-color: var(--text-secondary);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
		flex-shrink: 0;
	}
	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	/* ── Account indicator & dropdown ── */
	.tb-right-wrap {
		position: relative;
	}
	.account-dropdown-wrap {
		position: relative;
	}
	.account-backdrop {
		position: fixed;
	}
	.account-btn {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 4px 10px;
		border-radius: 6px;
		border: 1px solid var(--border);
		background: var(--bg-tertiary);
		cursor: pointer;
		font-size: 12px;
		color: var(--text-secondary);
	}
	.account-btn:hover {
		background: var(--bg-hover);
		color: var(--text-primary);
	}
	.account-btn.account-warning {
		border-color: var(--accent-red) !important;
		color: var(--accent-red) !important;
	}
	.account-avatar {
		width: 20px;
		height: 20px;
		border-radius: 50%;
		background: var(--accent-blue);
		color: #0a0a0a;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 11px;
		font-weight: 600;
	}
	.account-label {
		max-width: 100px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.account-btn .account-email {
		font-size: 10px;
		color: var(--text-muted);
		max-width: 80px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.account-menu {
		position: absolute;
		top: 100%;
		right: 0;
		margin-top: 4px;
		min-width: 280px;
		padding: 12px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 8px;
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
		z-index: 100;
	}
	.account-menu-header {
		font-size: 12px;
		font-weight: 600;
		color: var(--text-primary);
		margin-bottom: 12px;
		padding-bottom: 8px;
		border-bottom: 1px solid var(--border);
	}
	.current-repo-label {
		font-weight: 400;
		color: var(--text-muted);
	}
	.account-menu-current,
	.account-menu-profiles {
		margin-bottom: 12px;
	}
	.menu-label {
		display: block;
		font-size: 10px;
		text-transform: uppercase;
		letter-spacing: 0.5px;
		color: var(--text-muted);
		margin-bottom: 6px;
	}
	.account-row {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 8px 10px;
		border-radius: 6px;
		margin-bottom: 4px;
	}
	.account-row.active {
		background: var(--bg-secondary);
	}
	.account-row.clickable {
		cursor: pointer;
	}
	.account-row.clickable:hover {
		background: var(--bg-hover);
	}
	.account-avatar-lg {
		width: 32px;
		height: 32px;
		border-radius: 50%;
		background: var(--accent-blue);
		color: #0a0a0a;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 14px;
		font-weight: 600;
		flex-shrink: 0;
	}
	.account-info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	.account-info .account-name {
		font-size: 13px;
		font-weight: 500;
		color: var(--text-primary);
	}
	.account-email-small,
	.account-host,
	.account-key {
		font-size: 11px;
		color: var(--text-muted);
	}
	.account-key {
		font-size: 10px;
		opacity: 0.8;
	}
	.checkmark {
		color: var(--accent-green);
		font-size: 14px;
	}

	/* ── Switch modal & Push warning ── */
	.push-warning-overlay {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.6);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
	}
	.push-warning-modal {
		background: var(--bg-secondary);
		border: 1px solid var(--accent-red);
		border-radius: 8px;
		padding: 24px;
		width: 360px;
		text-align: center;
	}
	.warning-icon {
		font-size: 32px;
		margin-bottom: 12px;
	}
	.push-warning-modal h3 {
		color: var(--accent-red);
		margin-bottom: 12px;
	}
	.push-warning-modal p {
		color: var(--text-secondary);
		font-size: 13px;
		line-height: 1.6;
		margin-bottom: 8px;
	}
	.warning-sub {
		margin-top: 8px;
	}
	.warning-actions {
		display: flex;
		flex-direction: column;
		gap: 8px;
		margin-top: 16px;
	}
	.btn-danger {
		background: var(--accent-red);
		color: white;
		border: none;
		border-radius: 4px;
		padding: 8px;
		cursor: pointer;
		font-size: 13px;
	}
	.btn-secondary {
		background: var(--bg-tertiary);
		color: var(--accent-blue);
		border: 1px solid var(--accent-blue);
		border-radius: 4px;
		padding: 8px;
		cursor: pointer;
		font-size: 13px;
	}
	.btn-cancel {
		background: transparent;
		color: var(--text-muted);
		border: none;
		cursor: pointer;
		font-size: 12px;
		padding: 4px;
	}
	.btn-cancel:hover {
		color: var(--text-secondary);
	}

	/* Switch account modal */
	.switch-modal {
		background: var(--bg-secondary);
		border: 1px solid var(--border);
		border-radius: 8px;
		padding: 24px;
		width: 380px;
		text-align: left;
	}
	.switch-modal h3 {
		font-size: 16px;
		color: var(--text-primary);
		margin-bottom: 4px;
	}
	.switch-modal-sub {
		font-size: 12px;
		color: var(--text-muted);
		margin-bottom: 16px;
	}
	.switch-modal-form {
		display: flex;
		flex-direction: column;
		gap: 8px;
		margin-bottom: 16px;
	}
	.switch-modal-form label {
		font-size: 12px;
		color: var(--text-secondary);
	}
	.switch-modal-form input {
		padding: 8px 10px;
		border-radius: 4px;
		border: 1px solid var(--border);
		background: var(--bg-tertiary);
		color: var(--text-primary);
		font-size: 13px;
	}
	.switch-modal-note {
		font-size: 11px;
		color: var(--text-muted);
		line-height: 1.5;
		margin-bottom: 16px;
	}
	.switch-modal-actions {
		display: flex;
		gap: 8px;
		justify-content: flex-end;
	}
	.btn-primary {
		background: var(--accent-blue);
		color: #0a0a0a;
		border: none;
		border-radius: 4px;
		padding: 8px 16px;
		cursor: pointer;
		font-size: 13px;
		font-weight: 500;
	}
	.btn-primary:hover {
		opacity: 0.9;
	}
</style>
