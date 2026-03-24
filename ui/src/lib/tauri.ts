import { invoke } from '@tauri-apps/api/core';
import type {
	LanedCommit,
	BranchInfo,
	IndexEntry,
	StatusEntry,
	DiffFile,
	FetchResult,
	RepoRecord
} from './types';

export interface StashEntry {
	index: number;
	name: string;
	message: string;
	branch: string;
	date: string;
	hash: string;
}

export interface BlameLine {
	line_no: number;
	content: string;
	commit_hash: string;
	short_hash: string;
	author: string;
	author_email: string;
	date: string;
	timestamp: number;
	summary: string;
}

/** Parse JSON result from Tauri command, throw on error */
async function parseJson<T>(raw: unknown): Promise<T> {
	const str = typeof raw === 'string' ? raw : String(raw);
	return JSON.parse(str) as T;
}

export async function getCommits(
	repoPath: string,
	limit: number,
	offset: number
): Promise<LanedCommit[]> {
	const raw = await invoke('get_commits', {
		repoPath,
		limit,
		offset
	});
	return parseJson<LanedCommit[]>(raw);
}

export async function getBranches(repoPath: string): Promise<BranchInfo[]> {
	const raw = await invoke('get_branches', { repoPath });
	return parseJson<BranchInfo[]>(raw);
}

export async function getStatus(repoPath: string): Promise<StatusEntry[]> {
	const raw = await invoke('get_status', { repoPath });
	return parseJson<StatusEntry[]>(raw);
}

export async function stageFile(
	repoPath: string,
	filePath: string
): Promise<void> {
	await invoke('stage_file', { repoPath, filePath });
}

export async function unstageFile(
	repoPath: string,
	filePath: string
): Promise<void> {
	await invoke('unstage_file', { repoPath, filePath });
}

export async function unstageAll(repoPath: string): Promise<void> {
	await invoke('unstage_all', { repoPath });
}

export async function getGitConfig(
	repoPath: string,
	key: string
): Promise<string> {
	return invoke('get_git_config', { repoPath, key }) as Promise<string>;
}

export async function stageAll(repoPath: string): Promise<void> {
	await invoke('stage_all', { repoPath });
}

export async function createCommit(
	repoPath: string,
	message: string,
	authorName: string,
	authorEmail: string
): Promise<string> {
	return invoke('create_commit', {
		repoPath,
		message,
		authorName,
		authorEmail
	}) as Promise<string>;
}

export async function cherryPick(
	repoPath: string,
	commitHash: string
): Promise<string> {
	return invoke<string>('cherry_pick', { repoPath, commitHash });
}

export async function revertCommit(
	repoPath: string,
	commitHash: string
): Promise<string> {
	return invoke<string>('revert_commit', { repoPath, commitHash });
}

export async function resetToCommit(
	repoPath: string,
	commitHash: string,
	mode: 'soft' | 'mixed' | 'hard'
): Promise<string> {
	return invoke<string>('reset_to_commit', { repoPath, commitHash, mode });
}

export async function getDiffCommit(
	repoPath: string,
	commitHash: string
): Promise<DiffFile[]> {
	const raw = await invoke('get_diff_commit', {
		repoPath,
		commitHash
	});
	return parseJson<DiffFile[]>(raw);
}

export async function getDiffWorkingTree(
	repoPath: string
): Promise<DiffFile[]> {
	const raw = await invoke('get_diff_working_tree', { repoPath });
	return parseJson<DiffFile[]>(raw);
}

export async function getDiffStaged(repoPath: string): Promise<DiffFile[]> {
	const raw = await invoke('get_diff_staged', { repoPath });
	return parseJson<DiffFile[]>(raw);
}

export async function gitBlame(
	repoPath: string,
	filePath: string,
	commitHash?: string
): Promise<BlameLine[]> {
	const raw = await invoke<string>('git_blame', {
		repoPath,
		filePath,
		commitHash: commitHash ?? null
	});
	return parseJson<BlameLine[]>(raw);
}

export async function pull(
	repoPath: string,
	remoteName: string,
	branchName: string
): Promise<string> {
	return invoke('pull', {
		repoPath,
		remoteName,
		branchName
	}) as Promise<string>;
}

export async function push(
	repoPath: string,
	remoteName: string,
	branchName: string,
	force: boolean = false
): Promise<string> {
	return invoke('push', {
		repoPath,
		remoteName,
		branchName,
		force
	}) as Promise<string>;
}

export async function fetchRemote(
	repoPath: string,
	remoteName: string
): Promise<FetchResult> {
	const raw = await invoke('fetch_remote', {
		repoPath,
		remoteName
	});
	return parseJson<FetchResult>(raw);
}

export async function openRepository(repoPath: string): Promise<string> {
	return invoke('open_repository', { repoPath }) as Promise<string>;
}

export async function checkoutBranch(repoPath: string, name: string): Promise<void> {
	await invoke('checkout_branch', { repoPath, name });
}

export async function deleteBranch(
	repoPath: string,
	name: string,
	force: boolean = false
): Promise<void> {
	await invoke('delete_branch', { repoPath, name, force });
}

export async function renameBranch(
	repoPath: string,
	oldName: string,
	newName: string
): Promise<void> {
	await invoke('rename_branch', { repoPath, oldName, newName });
}

export async function createBranch(
	repoPath: string,
	name: string,
	fromRef: string
): Promise<void> {
	await invoke('create_branch', { repoPath, name, fromRef });
}

export async function mergeBranch(repoPath: string, branchName: string): Promise<void> {
	await invoke('merge_branch', { repoPath, branchName });
}

export async function listStashes(repoPath: string): Promise<StashEntry[]> {
	const raw = await invoke<string>('list_stashes', { repoPath });
	return JSON.parse(raw);
}

export async function stashPush(repoPath: string, message: string): Promise<string> {
	return invoke<string>('stash_push', { repoPath, message });
}

export async function stashPop(repoPath: string, index: number): Promise<string> {
	return invoke<string>('stash_pop', { repoPath, index });
}

export async function stashApply(repoPath: string, index: number): Promise<string> {
	return invoke<string>('stash_apply', { repoPath, index });
}

export async function stashDrop(repoPath: string, index: number): Promise<string> {
	return invoke<string>('stash_drop', { repoPath, index });
}

export async function stashBranch(
	repoPath: string,
	index: number,
	branchName: string
): Promise<string> {
	return invoke<string>('stash_branch', { repoPath, index, branchName });
}

export async function openTerminalAt(repoPath: string): Promise<void> {
	await invoke('open_terminal_at', { repoPath });
}

export async function getRecentRepositories(
	limit: number
): Promise<RepoRecord[]> {
	const raw = await invoke('get_recent_repositories', { limit });
	return parseJson<RepoRecord[]>(raw);
}

export async function discardFile(
	repoPath: string,
	filePath: string
): Promise<void> {
	await invoke('discard_file', { repoPath, filePath });
}

export async function discardAllChanges(repoPath: string): Promise<void> {
	await invoke('discard_all_changes', { repoPath });
}

export async function startFileWatch(repoPath: string): Promise<void> {
    await invoke('start_file_watch', { repoPath })
}

export async function stopFileWatch(): Promise<void> {
    await invoke('stop_file_watch')
}

export async function getRepoIdentity(repoPath: string) {
	const raw = await invoke<string>('get_repo_identity', { repoPath });
	return JSON.parse(raw);
}

export async function getSshProfiles() {
	const raw = await invoke<string>('get_ssh_profiles');
	return JSON.parse(raw);
}

export async function getSshUsername(hostAlias: string): Promise<string> {
	return invoke<string>('get_ssh_username', { hostAlias });
}

export async function switchRepoIdentity(
	repoPath: string,
	sshHostAlias: string,
	name: string,
	email: string
) {
	await invoke('switch_repo_identity', {
		repoPath,
		sshHostAlias,
		name,
		email
	});
}
