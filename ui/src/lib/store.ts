import { writable, get } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import {
	getCommits,
	getBranches,
	getTags,
	getStatus,
	getDiffCommit,
	getDiffWorkingTree,
	getDiffStaged,
	openRepository,
	fetchRemote,
	startFileWatch,
	stopFileWatch,
	listPrs,
	listIssues,
	detectPlatform,
	getRepoState,
	getGraphState,
	stashShow,
	getFileHistory,
	getFileDiffAtCommit,
	detectOperationState,
	getBisectState,
	startBisect as startBisectIpc,
	bisectGood as bisectGoodIpc,
	bisectBad as bisectBadIpc,
	bisectSkip as bisectSkipIpc,
	bisectReset as bisectResetIpc
} from './tauri';
import type { LanedCommit, BranchInfo, TagInfo, IndexEntry, DiffFile, StatusEntry, PullRequest, GitIssue, RepoCoords, FileHistoryEntry, RepoOperationState, BisectState } from './types';

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
const tagsStore = writable<TagInfo[]>([]);
const prsStore = writable<PullRequest[]>([]);
const issuesStore = writable<GitIssue[]>([]);
const repoCoordsStore = writable<RepoCoords | null>(null);
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
export type CenterView = 'graph' | 'diff' | 'file-history' | 'conflict' | 'rebase' | 'pr-review';
export type DiffMode = 'working-tree' | 'staged' | 'commit';
export type RightPanelMode = 'wip' | 'commit';

const centerViewStore = writable<CenterView>('graph');
const conflictFilePathStore = writable<string | null>(null);
export const conflictFilePath = { subscribe: conflictFilePathStore.subscribe };
const diffFileStore = writable<DiffFile | null>(null);
const diffModeStore = writable<DiffMode>('commit');
const rightPanelModeStore = writable<RightPanelMode>('wip');
const openCreateBranchFormStore = writable<boolean>(false);
const createBranchFromHashStore = writable<string | null>(null);

// Operation state (merge/rebase/cherry-pick in progress)
const operationStateStore = writable<RepoOperationState | null>(null);

// Bisect state
const bisectStateStore = writable<BisectState | null>(null);

// PR review state
const selectedPrStore = writable<number | null>(null); // PR number

// Multi-repo tabs
interface RepoTab {
	path: string;
	name: string;
}
const openTabsStore = writable<RepoTab[]>([]);
const activeTabIndexStore = writable<number>(0);

// File history state
const fileHistoryStore = writable<FileHistoryEntry[]>([]);
const fileHistoryPathStore = writable<string | null>(null);
const fileHistoryLoadingStore = writable<boolean>(false);

// ─── Commit diff cache (immutable data, safe to cache by hash) ─────
const commitDiffCache = new Map<string, DiffFile[]>();
const DIFF_CACHE_MAX_SIZE = 50;

function getCachedDiff(hash: string): DiffFile[] | undefined {
	return commitDiffCache.get(hash);
}

function setCachedDiff(hash: string, files: DiffFile[]): void {
	if (commitDiffCache.size >= DIFF_CACHE_MAX_SIZE) {
		// Evict oldest entry
		const firstKey = commitDiffCache.keys().next().value;
		if (firstKey) commitDiffCache.delete(firstKey);
	}
	commitDiffCache.set(hash, files);
}

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
export const tags = { subscribe: tagsStore.subscribe };
export const prs = { subscribe: prsStore.subscribe };
export const issues = { subscribe: issuesStore.subscribe };
export const repoCoords = { subscribe: repoCoordsStore.subscribe };
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
export const centerView = { subscribe: centerViewStore.subscribe, set: centerViewStore.set };
export const diffFile = { subscribe: diffFileStore.subscribe };
export const diffMode = { subscribe: diffModeStore.subscribe };
export const rightPanelMode = { subscribe: rightPanelModeStore.subscribe };
export const openCreateBranchForm = { subscribe: openCreateBranchFormStore.subscribe, set: openCreateBranchFormStore.set };
export const createBranchFromHash = { subscribe: createBranchFromHashStore.subscribe, set: createBranchFromHashStore.set };
export const isRefreshing = { subscribe: isRefreshingStore.subscribe };
export const operationState = { subscribe: operationStateStore.subscribe };
export const bisectState = { subscribe: bisectStateStore.subscribe };
export const selectedPr = { subscribe: selectedPrStore.subscribe };
export const openTabs = { subscribe: openTabsStore.subscribe };
export const activeTabIndex = { subscribe: activeTabIndexStore.subscribe };
export const fileHistory = { subscribe: fileHistoryStore.subscribe };
export const fileHistoryPath = { subscribe: fileHistoryPathStore.subscribe };
export const fileHistoryLoading = { subscribe: fileHistoryLoadingStore.subscribe };

async function loadGitHubData(repoPath: string): Promise<void> {
	try {
		const coords = await detectPlatform(repoPath);
		repoCoordsStore.set(coords);
		const [prData, issueData] = await Promise.all([
			listPrs(repoPath).catch(() => [] as PullRequest[]),
			listIssues(repoPath).catch(() => [] as GitIssue[]),
		]);
		prsStore.set(prData);
		issuesStore.set(issueData);
	} catch {
		// Not a GitHub/GitLab repo or no token — silently clear
		repoCoordsStore.set(null);
		prsStore.set([]);
		issuesStore.set([]);
	}
}

export async function refreshGitHub(): Promise<void> {
	const repo = get(currentRepoStore);
	if (repo) await loadGitHubData(repo);
}

/** Lazy-load GitHub data. Call from component onMount instead of blocking repo load. */
export async function lazyLoadGitHub(): Promise<void> {
	const repo = get(currentRepoStore);
	if (!repo) return;
	// Skip if already loaded
	const existing = get(repoCoordsStore);
	if (existing) return;
	await loadGitHubData(repo);
}

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

		// Add to tabs if not already open
		const tabs = get(openTabsStore);
		const existingIdx = tabs.findIndex(t => t.path === repoPath);
		const repoName = repoPath.split('/').pop() ?? repoPath;
		if (existingIdx === -1) {
			openTabsStore.set([...tabs, { path: repoPath, name: repoName }]);
			activeTabIndexStore.set(tabs.length);
		} else {
			activeTabIndexStore.set(existingIdx);
		}

		// Single IPC call replaces 4 separate calls
		const { commits, branches, tags, status } = await getRepoState(repoPath);

		commitsStore.set(commits);
		branchesStore.set(branches);
		tagsStore.set(tags);
		statusStore.set(status);
		hasMoreStore.set(true);
		commitDiffCache.clear();

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

/** Only show the WIP header refresh indicator if getStatus takes longer than this. */
const STATUS_REFRESH_INDICATOR_DELAY_MS = 280;
let statusRefreshDepth = 0;
let statusRefreshIndicatorTimer: ReturnType<typeof setTimeout> | null = null;

function armStatusRefreshIndicator(): void {
	if (statusRefreshIndicatorTimer !== null) return;
	statusRefreshIndicatorTimer = setTimeout(() => {
		statusRefreshIndicatorTimer = null;
		if (statusRefreshDepth > 0) {
			isRefreshingStore.set(true);
		}
	}, STATUS_REFRESH_INDICATOR_DELAY_MS);
}

function disarmStatusRefreshIndicator(): void {
	if (statusRefreshIndicatorTimer !== null) {
		clearTimeout(statusRefreshIndicatorTimer);
		statusRefreshIndicatorTimer = null;
	}
	isRefreshingStore.set(false);
}

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
	statusRefreshDepth += 1;
	if (statusRefreshDepth === 1) {
		armStatusRefreshIndicator();
	}
	try {
		statusStore.set(await getStatus(repo));
		// Also check for merge/rebase in progress
		refreshOperationState();
	} catch (err) {
		errorStore.set(err instanceof Error ? err.message : String(err));
	} finally {
		statusRefreshDepth -= 1;
		if (statusRefreshDepth === 0) {
			disarmStatusRefreshIndicator();
		}
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
		const newLimit = currentCommits.length + 500;
		const { commits, branches, tags } = await getGraphState(repo, newLimit);
		if (commits.length <= currentCommits.length) {
			hasMoreStore.set(false);
			return;
		}
		if (commits.length < newLimit) {
			hasMoreStore.set(false);
		}
		commitsStore.set(commits);
		branchesStore.set(branches);
		tagsStore.set(tags);
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

	// Check cache first — commit diffs are immutable
	const cached = getCachedDiff(commit.commit.hash);
	if (cached) {
		commitDiffFilesStore.set(cached);
		return;
	}

	isLoadingStore.set(true);
	try {
		const files = await getDiffCommit(repo, commit.commit.hash);
		setCachedDiff(commit.commit.hash, files);
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

/** Preview a stash entry — loads its diff and shows it in the right panel. */
export async function selectStash(stashIndex: number): Promise<void> {
	const repo = get(currentRepoStore);
	if (!repo) return;

	selectedCommitStore.set(null);
	rightPanelModeStore.set('commit');
	isLoadingStore.set(true);
	try {
		const files = await stashShow(repo, stashIndex);
		commitDiffFilesStore.set(files);
	} catch (err) {
		errorStore.set(err instanceof Error ? err.message : String(err));
		commitDiffFilesStore.set([]);
	} finally {
		isLoadingStore.set(false);
	}
}

/** Refresh the operation state (merge/rebase in progress?). */
export async function refreshOperationState(): Promise<void> {
	const repo = get(currentRepoStore);
	if (!repo) return;
	try {
		const state = await detectOperationState(repo);
		operationStateStore.set(state);
	} catch {
		operationStateStore.set(null);
	}
}

/** Open a PR for review in the center panel. */
export function openPrReview(prNumber: number): void {
	selectedPrStore.set(prNumber);
	centerViewStore.set('pr-review');
}

/** Switch to a tab by index. */
export async function switchTab(index: number): Promise<void> {
	const tabs = get(openTabsStore);
	if (index < 0 || index >= tabs.length) return;
	activeTabIndexStore.set(index);
	const tab = tabs[index];
	if (tab.path !== get(currentRepoStore)) {
		await loadRepo(tab.path);
	}
}

/** Close a tab by index. Switches to adjacent tab. */
export async function closeTab(index: number): Promise<void> {
	const tabs = get(openTabsStore);
	if (tabs.length <= 1) return; // Don't close last tab
	const newTabs = tabs.filter((_, i) => i !== index);
	openTabsStore.set(newTabs);
	const activeIdx = get(activeTabIndexStore);
	if (index === activeIdx) {
		const newIdx = Math.min(index, newTabs.length - 1);
		activeTabIndexStore.set(newIdx);
		await loadRepo(newTabs[newIdx].path);
	} else if (index < activeIdx) {
		activeTabIndexStore.set(activeIdx - 1);
	}
}

/** Refresh bisect state. */
export async function refreshBisectState(): Promise<void> {
	const repo = get(currentRepoStore);
	if (!repo) return;
	try {
		const state = await getBisectState(repo);
		bisectStateStore.set(state.active ? state : null);
	} catch {
		bisectStateStore.set(null);
	}
}

/** Start a bisect session. */
export async function startBisect(badHash: string, goodHash: string): Promise<void> {
	const repo = get(currentRepoStore);
	if (!repo) return;
	const state = await startBisectIpc(repo, badHash, goodHash);
	bisectStateStore.set(state);
}

/** Mark current bisect commit as good. */
export async function markBisectGood(): Promise<void> {
	const repo = get(currentRepoStore);
	if (!repo) return;
	const state = await bisectGoodIpc(repo);
	bisectStateStore.set(state);
}

/** Mark current bisect commit as bad. */
export async function markBisectBad(): Promise<void> {
	const repo = get(currentRepoStore);
	if (!repo) return;
	const state = await bisectBadIpc(repo);
	bisectStateStore.set(state);
}

/** Skip current bisect commit. */
export async function skipBisectCommit(): Promise<void> {
	const repo = get(currentRepoStore);
	if (!repo) return;
	const state = await bisectSkipIpc(repo);
	bisectStateStore.set(state);
}

/** End bisect session. */
export async function resetBisect(): Promise<void> {
	const repo = get(currentRepoStore);
	if (!repo) return;
	await bisectResetIpc(repo);
	bisectStateStore.set(null);
	await loadRepo(repo);
}

/** Open the conflict resolver for a specific file. */
export function openConflictResolver(filePath: string): void {
	conflictFilePathStore.set(filePath);
	centerViewStore.set('conflict');
}

/** Show the commit history for a file. Opens the file-history center view. */
export async function showFileHistory(filePath: string): Promise<void> {
	const repo = get(currentRepoStore);
	if (!repo) return;

	fileHistoryPathStore.set(filePath);
	fileHistoryStore.set([]);
	fileHistoryLoadingStore.set(true);
	centerViewStore.set('file-history');

	try {
		const entries = await getFileHistory(repo, filePath, 100);
		fileHistoryStore.set(entries);
	} catch (err) {
		errorStore.set(err instanceof Error ? err.message : String(err));
	} finally {
		fileHistoryLoadingStore.set(false);
	}
}

/** Load the diff for a file at a specific commit (from file history view). */
export async function selectFileHistoryEntry(filePath: string, commitHash: string): Promise<void> {
	const repo = get(currentRepoStore);
	if (!repo) return;

	isLoadingStore.set(true);
	try {
		const files = await getFileDiffAtCommit(repo, filePath, commitHash);
		if (files.length > 0) {
			diffFileStore.set(files[0]);
			diffModeStore.set('commit');
			selectedFileStore.set(files[0]);
			centerViewStore.set('diff');
		}
	} catch (err) {
		errorStore.set(err instanceof Error ? err.message : String(err));
	} finally {
		isLoadingStore.set(false);
	}
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
	tagsStore.set([]);
	prsStore.set([]);
	issuesStore.set([]);
	repoCoordsStore.set(null);
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
		// Single IPC call replaces 3 separate calls
		const { commits, branches, tags } = await getGraphState(repo);
		commitsStore.set(commits);
		branchesStore.set(branches);
		tagsStore.set(tags);
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
			// Single IPC call replaces 3 separate calls
			const currentLen = get(commitsStore).length;
			const limit = Math.max(500, currentLen + 100); // always load at least 500; keep all currently loaded + 100 more
			const { commits, branches, tags } = await getGraphState(repoPath, limit);
			commitsStore.set(commits);
			branchesStore.set(branches);
			tagsStore.set(tags);
			hasMoreStore.set(true); // Fix: reset pagination state after graph reload
		} catch (err) {
			errorStore.set(err instanceof Error ? err.message : String(err));
		} finally {
			graphReloadTimer = null;
		}
	}, 500);
}
