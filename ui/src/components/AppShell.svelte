<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import { get } from "svelte/store";
	import { getCurrentWindow } from "@tauri-apps/api/window";
	import CenterPanel from "./CenterPanel.svelte";
	import RightPanel from "./RightPanel.svelte";
	import BranchSidebar from "./BranchSidebar.svelte";
	import TabBar from "./TabBar.svelte";
	import Toast from "./Toast.svelte";
	import CommandPalette from "./CommandPalette.svelte";
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
		getSshUsername,
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
	let leftPanelOpen = $state(stored("gax-left-open", 1) === 1);
	let resizing = $state<"left" | "right" | null>(null);

	let paletteOpen = $state(false);

	function handleGlobalKey(e: KeyboardEvent) {
		if (!e.metaKey && !e.ctrlKey) return;
		const key = e.key.toLowerCase();
		// ⌘K — command palette
		if (key === 'k') { e.preventDefault(); paletteOpen = true; return; }
		// ⌘F — fetch
		if (key === 'f') { e.preventDefault(); fetchFromRemote('origin'); return; }
		// ⌘P — push  (skip if input focused)
		if (key === 'p' && !(document.activeElement instanceof HTMLInputElement || document.activeElement instanceof HTMLTextAreaElement)) {
			e.preventDefault();
			const repo = get(currentRepo);
			if (repo) import('$lib/tauri').then(({ push }) => push(repo, 'origin', ''));
			return;
		}
	}

	function startResize(which: "left" | "right") {
		resizing = which;
	}

	function toggleLeftPanel() {
		leftPanelOpen = !leftPanelOpen;
		store("gax-left-open", leftPanelOpen ? 1 : 0);
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
		const repo = get(currentRepo);
		if (!repo) return;
		const msg = prompt("Stash message (optional):") ?? "";
		try {
			await stashPush(repo, msg);
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
			await stashPop(repo, 0);
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
	let profileUsernames = $state<Record<string, string>>({});
	let showAccountMenu = $state(false);
	let showSwitchModal = $state(false);
	let selectedProfile = $state<SshProfile | null>(null);
	let switchName = $state("");
	let switchEmail = $state("");
	let showPushWarning = $state(false);
	let pendingPushIdentity = $state<GitIdentity | null>(null);

	// Pre-fill name from current identity
	$effect(() => {
		if (showSwitchModal) {
			switchName = identity?.name ?? "";
			switchEmail = ""; // user must enter email
		}
	});

	function loadProfileUsernames(profilesList: SshProfile[]) {
		for (const profile of profilesList) {
			getSshUsername(profile.host_alias)
				.then((username) => {
					profileUsernames = {
						...profileUsernames,
						[profile.host_alias]: username,
					};
				})
				.catch(() => {});
		}
	}

	$effect(() => {
		const repo = $currentRepo;
		if (repo) {
			getRepoIdentity(repo)
				.then((id) => {
					identity = id;
				})
				.catch(() => {
					identity = null;
				});
			getSshProfiles()
				.then((p) => {
					profiles = p;
					loadProfileUsernames(p);
				})
				.catch(() => {
					profiles = [];
				});
		} else {
			identity = null;
			profiles = [];
			profileUsernames = {};
		}
	});

	function clickOutside(node: HTMLElement, handler: () => void) {
		const handleClick = (e: MouseEvent) => {
			if (!node.contains(e.target as Node)) handler();
		};
		document.addEventListener("click", handleClick);
		return {
			destroy() {
				document.removeEventListener("click", handleClick);
			},
		};
	}

	function openSwitchModal(profile: SshProfile) {
		selectedProfile = profile;
		showSwitchModal = true;
	}

	async function confirmSwitch() {
		const repo = $currentRepo;
		const profile = selectedProfile;
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
			showToast("Pushed successfully", "success");
		} catch (e) {
			showToast(e instanceof Error ? e.message : String(e), "error");
		} finally {
			isLoading.set(false);
			showPushWarning = false;
		}
	}

	async function onPush() {
		const repo = $currentRepo;
		if (!repo || !currentBranchName) {
			showToast("No branch selected or detached HEAD", "error");
			return;
		}
		isLoading.set(true);

		try {
			// Get fresh identity check before push
			const id = await getRepoIdentity(repo);
			identity = id;

			// Show warning if account seems wrong
			if (!id.is_correct) {
				showPushWarning = true;
				pendingPushIdentity = id;
				isLoading.set(false);
				return;
			}

			// Account correct, proceed with push
			await executePush();
		} catch (e) {
			showToast(e instanceof Error ? e.message : String(e), "error");
			isLoading.set(false);
		}
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="app-shell" class:is-resizing={resizing !== null}>
	<TabBar />
	<!-- ═══ TOOLBAR ═══ -->
	<header class="toolbar">
		<!-- Left -->
		<div class="tb-left">
			<button
				class="tb-icon-btn"
				onclick={toggleLeftPanel}
				title={leftPanelOpen ? "Hide sidebar" : "Show sidebar"}
				aria-label={leftPanelOpen ? "Hide sidebar" : "Show sidebar"}
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
				>
					{#if leftPanelOpen}
						<rect x="3" y="4" width="18" height="16" rx="2"></rect>
						<line x1="9" y1="4" x2="9" y2="20"></line>
					{:else}
						<rect x="3" y="4" width="18" height="16" rx="2"></rect>
						<line x1="3" y1="4" x2="3" y2="20"></line>
					{/if}
				</svg>
			</button>
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
				<span class="tb-branch-name" title={currentBranchName}>{currentBranchName}</span>
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
						onclick={(e) => {
							e.stopPropagation();
							showAccountMenu = !showAccountMenu;
						}}
						title={identity ? `${identity.account_label} — ${identity.email}` : 'Loading...'}
					>
						<div
							class="account-avatar"
							style="background: {identity?.is_correct ? 'var(--accent-blue)' : 'var(--accent-red)'}"
						>
							{identity?.name?.[0]?.toUpperCase() ?? '?'}
						</div>
						<div class="account-text">
							<span class="account-label">
								{identity?.account_label ?? 'Loading...'}
							</span>
							<span class="account-email">
								{identity?.email ?? ''}
							</span>
						</div>
						{#if identity && !identity.is_correct}
							<span class="warning-badge" title="Wrong account for this repo">⚠</span>
						{/if}
					</button>
				{#if showAccountMenu}
					<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
					<div
						class="tb-dropdown-backdrop account-backdrop"
						onclick={() => (showAccountMenu = false)}
					></div>
					<div class="account-menu" use:clickOutside={() => (showAccountMenu = false)}>
						<div class="account-menu-title">Git Account</div>
						<div class="account-menu-repo">
							for <strong>{repoName}</strong>
						</div>

						{#each profiles as profile}
							{@const username = profileUsernames[profile.host_alias] ?? null}
							{@const displayName = username ?? profile.label}
							<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
							<div
								class="account-profile-row"
								class:active={profile.host_alias === identity?.ssh_host}
								onclick={() => {
									if (profile.host_alias !== identity?.ssh_host) {
										selectedProfile = profile;
										showSwitchModal = true;
										showAccountMenu = false;
									}
								}}
							>
								<div
									class="profile-avatar"
									style="background: {profile.host_alias === identity?.ssh_host
										? 'var(--accent-green)'
										: 'var(--accent-blue)'}"
								>
									{displayName[0]?.toUpperCase() ?? '?'}
								</div>
								<div class="profile-info">
									<span class="profile-label">{displayName}</span>
									{#if username}
										<span class="profile-host">@{username}</span>
									{:else}
										<span class="profile-host">{profile.host_alias}</span>
									{/if}
									<span class="profile-key">{profile.identity_file}</span>
								</div>
								{#if profile.host_alias === identity?.ssh_host}
									<span class="profile-active">✓ Active</span>
								{:else}
									<span class="profile-switch">Switch →</span>
								{/if}
							</div>
						{/each}
					</div>
				{/if}
			</div>
		</div>
	</header>

	<!-- ═══ MAIN BODY (flex row) ═══ -->
	<div class="main-body">
		<!-- ZONE 1: LEFT SIDEBAR -->
		{#if leftPanelOpen}
			<aside class="left-panel" style="width: {leftWidth}px;">
				<BranchSidebar />
			</aside>
		{/if}

		<!-- Left resize handle -->
		{#if leftPanelOpen}
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
		{/if}

		<!-- ZONE 2: CENTER -->
		<main class="center">
			<CenterPanel {leftPanelOpen} />
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
	{#if showSwitchModal && selectedProfile}
		<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
		<div class="modal-overlay" onclick={() => (showSwitchModal = false)}>
			<div class="modal" onclick={(e) => e.stopPropagation()}>
				<h3>Switch to {selectedProfile.label}</h3>
				<p class="modal-sub">
					This will update the remote URL and local git config for
					<strong>{repoName}</strong>
				</p>

				<div class="form-group">
					<label for="switch-name-input">Name</label>
					<input id="switch-name-input" bind:value={switchName} placeholder="Your name" class="form-input" />
				</div>

				<div class="form-group">
					<label for="switch-email-input">Email</label>
					<input
						id="switch-email-input"
						bind:value={switchEmail}
						placeholder="your@email.com"
						type="email"
						class="form-input"
					/>
				</div>

				<div class="modal-info">
					<span>SSH Key: {selectedProfile.identity_file}</span>
					<span>Remote will use: git@{selectedProfile.host_alias}:...</span>
				</div>

				<div class="modal-actions">
					<button
						class="btn-primary"
						disabled={!switchName || !switchEmail}
						onclick={confirmSwitch}
					>
						Switch Account
					</button>
					<button class="btn-cancel" onclick={() => (showSwitchModal = false)}>
						Cancel
					</button>
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

	<svelte:window onkeydown={handleGlobalKey} />
	{#if paletteOpen}
		<CommandPalette onclose={() => (paletteOpen = false)} />
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
		gap: 8px;
		padding: 4px 10px;
		border-radius: 6px;
		border: 1px solid var(--border);
		background: var(--bg-tertiary);
		cursor: pointer;
		color: var(--text-secondary);
		height: 32px;
	}
	.account-btn:hover {
		background: var(--bg-hover);
		color: var(--text-primary);
	}
	.account-warning {
		border-color: var(--accent-red) !important;
	}
	.account-avatar {
		width: 22px;
		height: 22px;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 11px;
		font-weight: 700;
		color: white;
		flex-shrink: 0;
	}
	.account-text {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 0px;
	}
	.account-label {
		font-size: 11px;
		font-weight: 600;
		color: var(--text-primary);
		line-height: 1.2;
	}
	.account-email {
		font-size: 10px;
		color: var(--text-muted);
		line-height: 1.2;
	}
	.warning-badge {
		color: var(--accent-red);
		font-size: 14px;
		margin-left: 4px;
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
	.account-menu-title {
		font-size: 12px;
		font-weight: 600;
		color: var(--text-primary);
		margin-bottom: 4px;
	}
	.account-menu-repo {
		font-size: 11px;
		color: var(--text-muted);
		margin-bottom: 12px;
		padding-bottom: 8px;
		border-bottom: 1px solid var(--border);
	}
	.account-profile-row {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 8px 12px;
		border-radius: 6px;
		cursor: pointer;
		margin-bottom: 4px;
	}
	.account-profile-row:hover {
		background: var(--bg-hover);
	}
	.account-profile-row.active {
		background: var(--bg-secondary);
		cursor: default;
	}
	.profile-avatar {
		width: 32px;
		height: 32px;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 14px;
		font-weight: 700;
		color: white;
		flex-shrink: 0;
	}
	.profile-info {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-width: 0;
	}
	.profile-label {
		font-size: 12px;
		font-weight: 600;
		color: var(--text-primary);
	}
	.profile-host,
	.profile-key {
		font-size: 10px;
		color: var(--text-muted);
	}
	.profile-active {
		font-size: 11px;
		color: var(--accent-green);
		font-weight: 600;
	}
	.profile-switch {
		font-size: 11px;
		color: var(--accent-blue);
		opacity: 0;
		transition: opacity 0.2s;
	}
	.account-profile-row:hover .profile-switch {
		opacity: 1;
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

	/* Modern Modals */
	.modal-overlay {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.7);
		backdrop-filter: blur(4px);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
	}
	.modal {
		background: var(--bg-secondary);
		border: 1px solid var(--border);
		border-radius: 12px;
		padding: 24px;
		width: 400px;
		box-shadow: 0 20px 40px rgba(0, 0, 0, 0.5);
	}
	.modal h3 {
		margin-bottom: 8px;
		font-size: 18px;
	}
	.modal-sub {
		font-size: 13px;
		color: var(--text-muted);
		margin-bottom: 20px;
		line-height: 1.5;
	}
	.form-group {
		margin-bottom: 16px;
	}
	.form-group label {
		display: block;
		font-size: 12px;
		color: var(--text-secondary);
		margin-bottom: 6px;
	}
	.form-input {
		width: 100%;
		padding: 10px 12px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--text-primary);
		font-size: 14px;
	}
	.form-input:focus {
		border-color: var(--accent-blue);
		outline: none;
	}
	.modal-info {
		background: var(--bg-tertiary);
		padding: 12px;
		border-radius: 6px;
		margin-bottom: 24px;
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: 11px;
		color: var(--text-muted);
	}
	.modal-actions {
		display: flex;
		justify-content: flex-end;
		gap: 12px;
	}
	.btn-primary {
		background: var(--accent-blue);
		color: white;
		border: none;
		border-radius: 6px;
		padding: 10px 20px;
		font-weight: 600;
		cursor: pointer;
	}
	.btn-primary:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
	.btn-cancel {
		background: transparent;
		color: var(--text-secondary);
		border: 1px solid var(--border);
		border-radius: 6px;
		padding: 10px 20px;
		cursor: pointer;
	}
	.btn-cancel:hover {
		background: var(--bg-hover);
	}
</style>
