import { writable, get } from 'svelte/store';
import {
	getCommits,
	getBranches,
	getStatus,
	getDiffCommit,
	getDiffWorkingTree,
	getDiffStaged,
	openRepository,
	fetchRemote
} from './tauri';
import type { LanedCommit, BranchInfo, IndexEntry, DiffFile } from './types';

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
const statusStore = writable<IndexEntry[]>([]);
const selectedCommitStore = writable<LanedCommit | null>(null);
const selectedFileStore = writable<DiffFile | null>(null);
const commitDiffFilesStore = writable<DiffFile[]>([]);
const isLoadingStore = writable<boolean>(false);
const isDiffLoadingStore = writable<boolean>(false);
const errorStore = writable<string | null>(null);

// Export as readable stores for components
export const currentRepo = { subscribe: currentRepoStore.subscribe };
export const commits = { subscribe: commitsStore.subscribe };
export const branches = { subscribe: branchesStore.subscribe };
export const status = { subscribe: statusStore.subscribe };
export const selectedCommit = { subscribe: selectedCommitStore.subscribe };
export const selectedFile = { subscribe: selectedFileStore.subscribe };
export const commitDiffFiles = { subscribe: commitDiffFilesStore.subscribe };
export const isLoading = { subscribe: isLoadingStore.subscribe };
export const isDiffLoading = { subscribe: isDiffLoadingStore.subscribe };
export const error = { subscribe: errorStore.subscribe };

export async function loadRepo(repoPath: string): Promise<void> {
	const loadStart = Date.now();
	const MIN_LOAD_MS = 300;

	isLoadingStore.set(true);
	errorStore.set(null);
	try {
		await openRepository(repoPath);
		currentRepoStore.set(repoPath);
		setStoredRepo(repoPath);

		const [commitsData, branchesData, statusData] = await Promise.all([
			getCommits(repoPath, 200, 0),
			getBranches(repoPath),
			getStatus(repoPath)
		]);

		commitsStore.set(commitsData);
		branchesStore.set(branchesData);
		statusStore.set(statusData);
		selectedCommitStore.set(null);
		selectedFileStore.set(null);
		commitDiffFilesStore.set([]);
	} catch (err) {
		errorStore.set(err instanceof Error ? err.message : String(err));
		throw err;
	} finally {
		const elapsed = Date.now() - loadStart;
		const remaining = Math.max(0, MIN_LOAD_MS - elapsed);
		setTimeout(() => {
			isLoadingStore.set(false);
		}, remaining);
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

export async function loadMoreCommits(): Promise<void> {
	const repo = get(currentRepoStore);
	const currentCommits = get(commitsStore);
	if (!repo) return;
	isLoadingStore.set(true);
	errorStore.set(null);
	try {
		const more = await getCommits(repo, 200, currentCommits.length);
		commitsStore.update((c) => [...c, ...more]);
	} catch (err) {
		errorStore.set(err instanceof Error ? err.message : String(err));
	} finally {
		isLoadingStore.set(false);
	}
}

export async function selectCommit(commit: LanedCommit): Promise<void> {
	selectedCommitStore.set(commit);
	selectedFileStore.set(null);
	commitDiffFilesStore.set([]);

	const repo = get(currentRepoStore);
	if (!repo) return;

	isLoadingStore.set(true);
	try {
		const files = await getDiffCommit(repo, commit.commit.hash);
		commitDiffFilesStore.set(files);
		if (files.length > 0) {
			selectedFileStore.set(files[0]);
		}
	} catch (err) {
		errorStore.set(err instanceof Error ? err.message : String(err));
		commitDiffFilesStore.set([]);
	} finally {
		isLoadingStore.set(false);
	}
}

export function selectFile(file: DiffFile): void {
	selectedFileStore.set(file);
}

export async function selectFileFromStaging(
	repoPath: string,
	filePath: string,
	isStaged: boolean
): Promise<void> {
	selectedCommitStore.set(null);
	commitDiffFilesStore.set([]);
	selectedFileStore.set({
		old_path: filePath,
		new_path: filePath,
		status: 'Modified',
		hunks: []
	});
	isDiffLoadingStore.set(true);
	try {
		const files = isStaged
			? await getDiffStaged(repoPath)
			: await getDiffWorkingTree(repoPath);
		const match = files.find(
			(f) => (f.new_path ?? f.old_path ?? '') === filePath
		);
		if (match) {
			selectedFileStore.set(match);
		}
	} catch (err) {
		errorStore.set(err instanceof Error ? err.message : String(err));
	} finally {
		isDiffLoadingStore.set(false);
	}
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
			getCommits(repo, 200, 0),
			getBranches(repo)
		]);
		commitsStore.set(commitsData);
		branchesStore.set(branchesData);
	} catch (err) {
		errorStore.set(err instanceof Error ? err.message : String(err));
		throw err;
	} finally {
		isLoadingStore.set(false);
	}
}
