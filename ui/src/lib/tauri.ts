import { invoke } from '@tauri-apps/api/core';
import type {
	LanedCommit,
	BranchInfo,
	TagInfo,
	IndexEntry,
	StatusEntry,
	DiffFile,
	FetchResult,
	RepoRecord,
	RepoCoords,
	PullRequest,
	GitIssue,
	RepoStateResponse,
	GraphStateResponse,
	FileHistoryEntry,
	RemoteInfo,
	WorktreeInfo,
	ConflictFile,
	RepoOperationState,
	RebaseTodoItem,
	RebaseState,
	BisectState,
	ReviewComment,
	PrFile
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

export async function getTags(repoPath: string): Promise<TagInfo[]> {
	const raw = await invoke('list_tags', { repoPath });
	return parseJson<TagInfo[]>(raw);
}

export async function createTag(
	repoPath: string,
	name: string,
	targetHash: string,
	message: string,
): Promise<void> {
	await invoke('create_tag', { repoPath, name, targetHash, message });
}

export async function deleteTag(repoPath: string, name: string): Promise<void> {
	await invoke('delete_tag', { repoPath, name });
}

export async function pushTag(
	repoPath: string,
	remoteName: string,
	tagName: string,
): Promise<string> {
	return invoke('push_tag', { repoPath, remoteName, tagName }) as Promise<string>;
}

export async function deleteRemoteTag(
	repoPath: string,
	remoteName: string,
	tagName: string,
): Promise<string> {
	return invoke('delete_remote_tag', { repoPath, remoteName, tagName }) as Promise<string>;
}

// ─── GitHub/GitLab Integration ──────────────────────────────────────

export async function detectPlatform(repoPath: string): Promise<RepoCoords> {
	const raw = await invoke('detect_platform', { repoPath });
	return parseJson<RepoCoords>(raw);
}

export async function listPrs(repoPath: string): Promise<PullRequest[]> {
	const raw = await invoke('list_prs', { repoPath });
	return parseJson<PullRequest[]>(raw);
}

export async function listIssues(repoPath: string): Promise<GitIssue[]> {
	const raw = await invoke('list_issues', { repoPath });
	return parseJson<GitIssue[]>(raw);
}

export async function createPr(
	repoPath: string,
	title: string,
	body: string,
	head: string,
	base: string,
): Promise<PullRequest> {
	const raw = await invoke('create_pr', { repoPath, title, body, head, base });
	return parseJson<PullRequest>(raw);
}

export async function setApiToken(platform: string, token: string): Promise<void> {
	await invoke('set_api_token', { platform, token });
}

export async function getApiToken(platform: string): Promise<string> {
	return invoke('get_api_token', { platform }) as Promise<string>;
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

export async function stageHunk(repoPath: string, patchText: string): Promise<void> {
	await invoke('stage_hunk', { repoPath, patchText });
}

export async function unstageHunk(repoPath: string, patchText: string): Promise<void> {
	await invoke('unstage_hunk', { repoPath, patchText });
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
	authorEmail: string,
	amend: boolean = false
): Promise<string> {
	return invoke('create_commit', {
		repoPath,
		message,
		authorName,
		authorEmail,
		amend
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

export async function readDiffFileContent(
	repoPath: string,
	filePath: string,
	mode: 'working-tree' | 'staged' | 'commit',
	commitHash: string | null,
	fileStatus: DiffFile['status'] | null
): Promise<string> {
	return invoke<string>('read_diff_file_content', {
		repoPath,
		filePath,
		mode,
		commitHash,
		fileStatus
	});
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

export async function stashShow(repoPath: string, index: number): Promise<DiffFile[]> {
	const raw = await invoke<string>('stash_show', { repoPath, index });
	return JSON.parse(raw) as DiffFile[];
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

// ─── Batch commands (fewer IPC round-trips) ────────────────────────

/** Single IPC call to load all repo data. Replaces getCommits+getBranches+getTags+getStatus. */
export async function getRepoState(repoPath: string, limit?: number): Promise<{
	commits: LanedCommit[];
	branches: BranchInfo[];
	tags: TagInfo[];
	status: StatusEntry[];
}> {
	const raw = await invoke<RepoStateResponse>('get_repo_state', { repoPath, limit });
	return {
		commits: JSON.parse(raw.commits) as LanedCommit[],
		branches: JSON.parse(raw.branches) as BranchInfo[],
		tags: JSON.parse(raw.tags) as TagInfo[],
		status: JSON.parse(raw.status) as StatusEntry[],
	};
}

/** Single IPC call to reload graph after git events. Replaces getCommits+getBranches+getTags. */
export async function getGraphState(repoPath: string, limit?: number): Promise<{
	commits: LanedCommit[];
	branches: BranchInfo[];
	tags: TagInfo[];
}> {
	const raw = await invoke<GraphStateResponse>('get_graph_state', { repoPath, limit });
	return {
		commits: JSON.parse(raw.commits) as LanedCommit[],
		branches: JSON.parse(raw.branches) as BranchInfo[],
		tags: JSON.parse(raw.tags) as TagInfo[],
	};
}

// ─── File History ──────────────────────────────────────────────────

export async function getFileHistory(repoPath: string, filePath: string, limit: number = 100): Promise<FileHistoryEntry[]> {
	const raw = await invoke<string>('file_history', { repoPath, filePath, limit });
	return JSON.parse(raw) as FileHistoryEntry[];
}

export async function getFileDiffAtCommit(repoPath: string, filePath: string, commitHash: string): Promise<DiffFile[]> {
	const raw = await invoke<string>('file_diff_at_commit', { repoPath, filePath, commitHash });
	return JSON.parse(raw) as DiffFile[];
}

// ─── Remote Management ────────────────────────────────────────────

export async function listRemotes(repoPath: string): Promise<RemoteInfo[]> {
	const raw = await invoke<string>('list_remotes', { repoPath });
	return JSON.parse(raw) as RemoteInfo[];
}

export async function addRemote(repoPath: string, name: string, url: string): Promise<void> {
	await invoke('add_remote', { repoPath, name, url });
}

export async function removeRemote(repoPath: string, name: string): Promise<void> {
	await invoke('remove_remote', { repoPath, name });
}

export async function renameRemote(repoPath: string, oldName: string, newName: string): Promise<void> {
	await invoke('rename_remote', { repoPath, oldName, newName });
}

export async function setRemoteUrl(repoPath: string, name: string, url: string): Promise<void> {
	await invoke('set_remote_url', { repoPath, name, url });
}

// ─── Worktree Management ──────────────────────────────────────────

export async function listWorktrees(repoPath: string): Promise<WorktreeInfo[]> {
	const raw = await invoke<string>('list_worktrees', { repoPath });
	return JSON.parse(raw) as WorktreeInfo[];
}

export async function addWorktree(repoPath: string, path: string, branch: string): Promise<string> {
	return invoke<string>('add_worktree', { repoPath, path, branch });
}

export async function removeWorktree(repoPath: string, path: string, force: boolean = false): Promise<string> {
	return invoke<string>('remove_worktree', { repoPath, path, force });
}

// ─── Graph Search ─────────────────────────────────────────────────

export async function searchCommits(
	repoPath: string,
	opts: { query?: string; author?: string; since?: string; until?: string; path?: string; limit?: number }
): Promise<LanedCommit[]> {
	const raw = await invoke<string>('search_commits', {
		repoPath,
		query: opts.query ?? null,
		author: opts.author ?? null,
		since: opts.since ?? null,
		until: opts.until ?? null,
		path: opts.path ?? null,
		limit: opts.limit ?? 500,
	});
	return JSON.parse(raw) as LanedCommit[];
}

// ─── Conflict Resolution ──────────────────────────────────────────

export async function detectOperationState(repoPath: string): Promise<RepoOperationState> {
	const raw = await invoke<string>('detect_operation_state', { repoPath });
	return JSON.parse(raw) as RepoOperationState;
}

export async function getConflictFile(repoPath: string, filePath: string): Promise<ConflictFile> {
	const raw = await invoke<string>('get_conflict_file', { repoPath, filePath });
	return JSON.parse(raw) as ConflictFile;
}

export async function resolveConflict(repoPath: string, filePath: string, resolvedContent: string): Promise<void> {
	await invoke('resolve_conflict', { repoPath, filePath, resolvedContent });
}

export async function continueOperation(repoPath: string): Promise<string> {
	return invoke<string>('continue_operation', { repoPath });
}

export async function abortOperation(repoPath: string): Promise<string> {
	return invoke<string>('abort_operation', { repoPath });
}

// ─── Interactive Rebase ───────────────────────────────────────────

export async function getRebaseState(repoPath: string): Promise<RebaseState> {
	const raw = await invoke<string>('get_rebase_state', { repoPath });
	return JSON.parse(raw) as RebaseState;
}

export async function getRebaseTodoForRange(repoPath: string, onto: string): Promise<RebaseTodoItem[]> {
	const raw = await invoke<string>('get_rebase_todo_for_range', { repoPath, onto });
	return JSON.parse(raw) as RebaseTodoItem[];
}

export async function startInteractiveRebase(repoPath: string, onto: string, todo: RebaseTodoItem[]): Promise<string> {
	return invoke<string>('start_interactive_rebase', { repoPath, onto, todo: JSON.stringify(todo) });
}

export async function continueRebase(repoPath: string): Promise<string> {
	return invoke<string>('continue_rebase', { repoPath });
}

export async function abortRebase(repoPath: string): Promise<string> {
	return invoke<string>('abort_rebase', { repoPath });
}

export async function skipRebase(repoPath: string): Promise<string> {
	return invoke<string>('skip_rebase', { repoPath });
}

// ─── Git Bisect ───────────────────────────────────────────────────

export async function getBisectState(repoPath: string): Promise<BisectState> {
	const raw = await invoke<string>('get_bisect_state', { repoPath });
	return JSON.parse(raw) as BisectState;
}

export async function startBisect(repoPath: string, badHash: string, goodHash: string): Promise<BisectState> {
	const raw = await invoke<string>('start_bisect', { repoPath, badHash, goodHash });
	return JSON.parse(raw) as BisectState;
}

export async function bisectGood(repoPath: string): Promise<BisectState> {
	const raw = await invoke<string>('bisect_good', { repoPath });
	return JSON.parse(raw) as BisectState;
}

export async function bisectBad(repoPath: string): Promise<BisectState> {
	const raw = await invoke<string>('bisect_bad', { repoPath });
	return JSON.parse(raw) as BisectState;
}

export async function bisectSkip(repoPath: string): Promise<BisectState> {
	const raw = await invoke<string>('bisect_skip', { repoPath });
	return JSON.parse(raw) as BisectState;
}

export async function bisectReset(repoPath: string): Promise<string> {
	return invoke<string>('bisect_reset', { repoPath });
}

// ─── PR Review ────────────────────────────────────────────────────

export async function getPrComments(repoPath: string, prNumber: number): Promise<ReviewComment[]> {
	const raw = await invoke<string>('get_pr_comments', { repoPath, prNumber });
	return JSON.parse(raw) as ReviewComment[];
}

export async function getPrFiles(repoPath: string, prNumber: number): Promise<PrFile[]> {
	const raw = await invoke<string>('get_pr_files', { repoPath, prNumber });
	return JSON.parse(raw) as PrFile[];
}

export async function submitPrReview(repoPath: string, prNumber: number, body: string, event: string): Promise<string> {
	return invoke<string>('submit_pr_review', { repoPath, prNumber, body, event });
}

export async function cloneRepo(url: string, dest: string): Promise<void> {
	await invoke('clone_repo', { url, dest });
}
