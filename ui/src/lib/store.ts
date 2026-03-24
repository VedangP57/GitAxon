import { writable, get } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import {
	getCommits,
	getBranches,
	getStatus,
	getDiffCommit,
	getDiffWorkingTree,
	getDiffStaged,
	openRepository,
	fetchRemote,
	startFileWatch,
	stopFileWatch
} from './tauri';
import type { LanedCommit, BranchInfo, IndexEntry, DiffFile, StatusEntry } from './types';

const REPO_KEY = 'gitfast-current-repo';

function getStoredRepo(): string | null {
	if (typeof window === 'undefined') return null;
	try {
		return sessionStorage.getItem(REPO_KEY);
	} catch {
		return null;
	}
}

function setStoredRepo(path: string | null): void {
	try {
		if (path) sessionStorage.setItem(REPO_KEY, path);
		else sessionStorage.removeItem(REPO_KEY);
	} catch {
		// ignore
	}
}

// Writable stores (work in .ts, no rune restrictions)
const currentRepoStore = writable<string | null>(getStoredRepo());
const commitsStore = writable<LanedCommit[]>([]);
const branchesStore = writable<BranchInfo[]>([]);
// Legacy status store (IndexEntry) used by parts of the UI not yet migrated.
const legacyStatusStore = writable<IndexEntry[]>([]);
// Fast status store powered by Rust-owned state and patches.
const statusStore = writable<StatusEntry[]>([]);
const selectedCommitStore = writable<LanedCommit | null>(null);
const selectedFileStore = writable<DiffFile | null>(null);
const commitDiffFilesStore = writable<DiffFile[]>([]);
const isLoadingStore = writable<boolean>(false);
const isRefreshingStore = writable<boolean>(false);
const isDiffLoadingStore = writable<boolean>(false);
const errorStore = writable<string | null>(null);
const hasMoreStore = writable<boolean>(true);

// GitKraken 3-zone UI state
export type CenterView = 'graph' | 'diff';
export type DiffMode = 'working-tree' | 'staged' | 'commit';
export type RightPanelMode = 'wip' | 'commit';

const centerViewStore = writable<CenterView>('graph');
const diffFileStore = writable<DiffFile | null>(null);
const diffModeStore = writable<DiffMode>('commit');
const rightPanelModeStore = writable<RightPanelMode>('wip');
const openCreateBranchFormStore = writable<boolean>(false);
const createBranchFromHashStore = writable<string | null>(null);

// Guards to prevent duplicate repo loads / watchers
let loadRepoLock = false;
let currentLoadingRepo = '';
let watchedRepo = '';
let unlistenWorktree: (() => void) | null = null;
let unlistenGitState: (() => void) | null = null;

// Export as readable stores for components
export const currentRepo = { subscribe: currentRepoStore.subscribe };
export const commits = { subscribe: commitsStore.subscribe };
export const branches = { subscribe: branchesStore.subscribe };
export const status = {
	subscribe: statusStore.subscribe,
	set: statusStore.set,
	update: statusStore.update
};
export const selectedCommit = { subscribe: selectedCommitStore.subscribe };
export const selectedFile = { subscribe: selectedFileStore.subscribe };
export const commitDiffFiles = { subscribe: commitDiffFilesStore.subscribe };
export const isLoading = {
	subscribe: isLoadingStore.subscribe,
	set: isLoadingStore.set
};
export const isDiffLoading = { subscribe: isDiffLoadingStore.subscribe };
export const error = { subscribe: errorStore.subscribe };
export const hasMore = { subscribe: hasMoreStore.subscribe };
export const centerView = { subscribe: centerViewStore.subscribe };
export const diffFile = { subscribe: diffFileStore.subscribe };
export const diffMode = { subscribe: diffModeStore.subscribe };
export const rightPanelMode = { subscribe: rightPanelModeStore.subscribe };
export const openCreateBranchForm = { subscribe: openCreateBranchFormStore.subscribe, set: openCreateBranchFormStore.set };
export const createBranchFromHash = { subscribe: createBranchFromHashStore.subscribe, set: createBranchFromHashStore.set };
export const isRefreshing = { subscribe: isRefreshingStore.subscribe };

export function showWip(): void {
	selectedCommitStore.set(null);
	selectedFileStore.set(null);
	commitDiffFilesStore.set([]);
	diffFileStore.set(null);
	centerViewStore.set('graph');
	rightPanelModeStore.set('wip');
}

export async function loadRepo(repoPath: string): Promise<void> {
	// Skip if same repo already loaded or loading
	if (currentLoadingRepo === repoPath) {
		console.log('[Store] Skipping duplicate loadRepo for', repoPath);
		return;
	}

	if (loadRepoLock) {
		console.log('[Store] loadRepo already in progress, skipping');
		return;
	}

	loadRepoLock = true;
	currentLoadingRepo = repoPath;
	console.log('[Store] loadRepo START:', repoPath);

	const loadStart = Date.now();
	const MIN_LOAD_MS = 300;

	isLoadingStore.set(true);
	errorStore.set(null);
	try {
		await openRepository(repoPath);
		currentRepoStore.set(repoPath);
		setStoredRepo(repoPath);

		const [commitsData, branchesData, statusData] = await Promise.all([
			getCommits(repoPath, 500, 0),
			getBranches(repoPath),
			getStatus(repoPath)
		]);

		commitsStore.set(commitsData);
		branchesStore.set(branchesData);
		statusStore.set(statusData);
		hasMoreStore.set(true);
		selectedCommitStore.set(null);
		selectedFileStore.set(null);
		commitDiffFilesStore.set([]);

		await setupFileWatcher(repoPath);
	} catch (err) {
		console.error('[Store] loadRepo error:', err);
		errorStore.set(err instanceof Error ? err.message : String(err));
		throw err;
	} finally {
		const elapsed = Date.now() - loadStart;
		const remaining = Math.max(0, MIN_LOAD_MS - elapsed);
		setTimeout(() => {
			isLoadingStore.set(false);
		}, remaining);
		loadRepoLock = false;
		currentLoadingRepo = '';
	}
}

let refreshTimer: ReturnType<typeof setTimeout> | null = null;

export function debouncedRefreshStatus(): void {
	if (refreshTimer) clearTimeout(refreshTimer);
	refreshTimer = setTimeout(() => {
		refreshStatus();
		refreshTimer = null;
	}, 200);
}

export async function refreshStatus(): Promise<void> {
	const repo = get(currentRepoStore);
	if (!repo) return;
	try {
		statusStore.set(await getStatus(repo));
	} catch (err) {
		errorStore.set(err instanceof Error ? err.message : String(err));
	}
}

let isLoadingMore = false;

export async function loadMoreCommits(): Promise<void> {
	if (isLoadingMore || !get(hasMoreStore)) return;
	const repo = get(currentRepoStore);
	const currentCommits = get(commitsStore);
	if (!repo) return;
	isLoadingMore = true;
	errorStore.set(null);
	try {
		const more = await getCommits(repo, 500, currentCommits.length);
		if (more.length === 0) {
			hasMoreStore.set(false);
			return;
		}
		commitsStore.update((c) => [...c, ...more]);
	} catch (err) {
		errorStore.set(err instanceof Error ? err.message : String(err));
	} finally {
		setTimeout(() => {
			isLoadingMore = false;
		}, 300);
	}
}

export async function selectCommit(commit: LanedCommit): Promise<void> {
	selectedCommitStore.set(commit);
	commitDiffFilesStore.set([]);
	// Do NOT change centerView — stay on graph
	rightPanelModeStore.set('commit');

	const repo = get(currentRepoStore);
	if (!repo) return;

	isLoadingStore.set(true);
	try {
		const files = await getDiffCommit(repo, commit.commit.hash);
		commitDiffFilesStore.set(files);
	} catch (err) {
		errorStore.set(err instanceof Error ? err.message : String(err));
		commitDiffFilesStore.set([]);
	} finally {
		isLoadingStore.set(false);
	}
}

export function selectFile(file: DiffFile, mode: DiffMode = 'commit'): void {
	diffFileStore.set(file);
	diffModeStore.set(mode);
	centerViewStore.set('diff');
	// Keep legacy store in sync for components not yet migrated
	selectedFileStore.set(file);
}

export function closeDiff(): void {
	centerViewStore.set('graph');
	diffFileStore.set(null);
	selectedFileStore.set(null);
}

export async function selectFileFromStaging(
	repoPath: string,
	filePath: string,
	isStaged: boolean
): Promise<void> {
	selectedCommitStore.set(null);
	commitDiffFilesStore.set([]);
	const mode: DiffMode = isStaged ? 'staged' : 'working-tree';
	const placeholder: DiffFile = {
		old_path: filePath,
		new_path: filePath,
		status: 'Modified',
		hunks: []
	};
	selectFile(placeholder, mode);
	isDiffLoadingStore.set(true);
	try {
		const files = isStaged
			? await getDiffStaged(repoPath)
			: await getDiffWorkingTree(repoPath);
		const match = files.find(
			(f) => (f.new_path ?? f.old_path ?? '') === filePath
		);
		if (match) {
			selectFile(match, mode);
		}
	} catch (err) {
		errorStore.set(err instanceof Error ? err.message : String(err));
	} finally {
		isDiffLoadingStore.set(false);
	}
}

export function goHome(): void {
	watchedRepo = '';
	if (unlistenWorktree) {
		unlistenWorktree();
		unlistenWorktree = null;
	}
	if (unlistenGitState) {
		unlistenGitState();
		unlistenGitState = null;
	}
	try {
		void stopFileWatch();
	} catch {
		// ignore
	}

	currentRepoStore.set(null);
	setStoredRepo(null);
	commitsStore.set([]);
	branchesStore.set([]);
	statusStore.set([]);
	selectedCommitStore.set(null);
	selectedFileStore.set(null);
	commitDiffFilesStore.set([]);
	errorStore.set(null);
	centerViewStore.set('graph');
	diffFileStore.set(null);
	rightPanelModeStore.set('wip');
}

export function clearSelection(): void {
	selectedCommitStore.set(null);
	selectedFileStore.set(null);
	commitDiffFilesStore.set([]);
}

export async function fetchFromRemote(remoteName: string = 'origin'): Promise<void> {
	const repo = get(currentRepoStore);
	if (!repo) return;
	isLoadingStore.set(true);
	errorStore.set(null);
	try {
		await fetchRemote(repo, remoteName);
		const [commitsData, branchesData] = await Promise.all([
			getCommits(repo, 500, 0),
			getBranches(repo)
		]);
		commitsStore.set(commitsData);
		branchesStore.set(branchesData);
		hasMoreStore.set(true);
	} catch (err) {
		errorStore.set(err instanceof Error ? err.message : String(err));
		throw err;
	} finally {
		isLoadingStore.set(false);
	}
}

async function setupFileWatcher(repoPath: string) {
	// HARD GUARD — skip if already watching this repo
	if (watchedRepo === repoPath) {
		console.log('[Watcher] Already watching', repoPath);
		return;
	}
	watchedRepo = repoPath;

	// Clean up previous listeners
	if (unlistenWorktree) {
		unlistenWorktree();
		unlistenWorktree = null;
	}
	if (unlistenGitState) {
		unlistenGitState();
		unlistenGitState = null;
	}

	// Stop previous Rust watcher
	await stopFileWatch();

	// Start new Rust watcher
	await startFileWatch(repoPath);
	console.log('[Watcher] Started for', repoPath);

	// Listen for working tree changes (file saves) via status patches
	unlistenWorktree = await listen('worktree-changed', async (event) => {
		const current = get(currentRepoStore);
		if (!current) return;

		const raw = event.payload as string;
		console.log('[Frontend] worktree-changed RECEIVED:', raw);

		// Ignore initial test event
		if (raw === 'TEST_EVENT') {
			return;
		}

		try {
			const patch = JSON.parse(raw) as {
				added?: StatusEntry[];
				removed?: string[];
				changed?: StatusEntry[];
			};

			if (!patch || typeof patch !== 'object') return;

			const added = patch.added ?? [];
			const removed = patch.removed ?? [];
			const changed = patch.changed ?? [];

			// Skip empty patches (no real changes)
			if (added.length === 0 && removed.length === 0 && changed.length === 0) {
				return;
			}

			console.log(
				'[Watcher] Applying patch:',
				`+${added.length} -${removed.length} ~${changed.length}`
			);

			statusStore.update((currentStatus) => {
				let updated = [...currentStatus];

				// Remove files no longer modified
				if (removed.length > 0) {
					updated = updated.filter((e) => !removed.includes(e.path));
				}

				// Add new modified files
				for (const entry of added) {
					if (!updated.some((e) => e.path === entry.path)) {
						updated.push(entry);
					}
				}

				// Update changed files
				for (const entry of changed) {
					const idx = updated.findIndex((e) => e.path === entry.path);
					if (idx >= 0) {
						updated[idx] = entry;
					} else {
						updated.push(entry);
					}
				}

				return updated;
			});
		} catch (e) {
			console.error('[Watcher] Failed to parse patch:', e);
			// Fallback: full refresh
			getStatus(current).then((real) => statusStore.set(real));
		}
	});
	console.log('[Watcher] worktree-changed listener registered');

	// Listen for git state changes (commits, checkouts)
	unlistenGitState = await listen('git-state-changed', async () => {
		const current = get(currentRepoStore);
		if (!current) return;

		console.log('[Watcher] Git state changed');
		await refreshStatus();
		scheduleGraphReload(current);
	});
	console.log('[Watcher] git-state-changed listener registered');
}

let graphReloadTimer: ReturnType<typeof setTimeout> | null = null;

function scheduleGraphReload(repoPath: string) {
	if (graphReloadTimer) clearTimeout(graphReloadTimer);
	graphReloadTimer = setTimeout(async () => {
		try {
			const commitsData = await getCommits(repoPath, 500, 0);
			commitsStore.set(commitsData);
			const branchesData = await getBranches(repoPath);
			branchesStore.set(branchesData);
		} catch (err) {
			errorStore.set(err instanceof Error ? err.message : String(err));
		} finally {
			graphReloadTimer = null;
		}
	}, 500);
}
