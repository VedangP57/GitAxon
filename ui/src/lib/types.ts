export interface CommitNode {
	hash: string;
	short_hash: string;
	message: string;
	author_name: string;
	author_email: string;
	timestamp: number;
	parent_hashes: string[];
}

export interface Edge {
	from_lane: number;
	to_lane: number;
	color_index: number;
	edge_type: 'Straight' | 'Fork' | 'Merge';
}

export interface LanedCommit {
	commit: CommitNode;
	lane: number;
	color_index: number;
	edges: Edge[];
}

export interface BranchInfo {
	name: string;
	isRemote: boolean;
	isHead: boolean;
	upstream: string | null;
	tipHash: string;
	tipMessage: string;
}

export interface TagInfo {
	name: string;
	hash: string;
	message: string;
	isAnnotated: boolean;
	taggerName: string;
	taggerDate: number;
}

export interface DiffLine {
	content: string;
	line_type: 'Added' | 'Deleted' | 'Context';
	old_line_no: number | null;
	new_line_no: number | null;
}

export interface DiffHunk {
	old_start: number;
	old_lines: number;
	new_start: number;
	new_lines: number;
	lines: DiffLine[];
}

export interface DiffFile {
	old_path: string | null;
	new_path: string | null;
	status: 'Added' | 'Deleted' | 'Modified' | 'Renamed' | 'Copied';
	hunks: DiffHunk[];
}

export interface IndexEntry {
	path: string;
	status: 'Staged' | 'Unstaged' | 'Untracked' | 'Conflicted' | 'Ignored';
	staged_diff: DiffHunk[] | null;
	unstaged_diff: DiffHunk[] | null;
}

// Fast status entry driven directly by Rust FileStatus patches
export interface StatusEntry {
	path: string;
	status: string; // "M", "A", "D", "WM", "WD", "?"
	staged: boolean;
}

export interface StatusPatch {
	added: StatusEntry[];
	removed: string[];
	changed: StatusEntry[];
}

export interface FetchResult {
	remote: string;
	updatedRefs: string[];
	newRefs: string[];
}

export interface RepoCoords {
	platform: 'github' | 'gitlab' | 'unknown';
	owner: string;
	repo: string;
	apiBase: string;
}

export interface PullRequest {
	number: number;
	title: string;
	state: string;
	author: string;
	headBranch: string;
	baseBranch: string;
	createdAt: string;
	updatedAt: string;
	url: string;
	draft: boolean;
	additions: number;
	deletions: number;
	comments: number;
}

export interface GitIssue {
	number: number;
	title: string;
	state: string;
	author: string;
	labels: string[];
	createdAt: string;
	updatedAt: string;
	url: string;
}

export interface RepoRecord {
	id: string;
	path: string;
	name: string;
	last_opened: number;
	is_favorite: boolean;
}

/** Batch response from get_repo_state (single IPC call for repo load) */
export interface RepoStateResponse {
	commits: string;
	branches: string;
	tags: string;
	status: string;
}

/** Batch response from get_graph_state (single IPC call for graph reload) */
export interface GraphStateResponse {
	commits: string;
	branches: string;
	tags: string;
}

/** A commit entry in file history */
export interface FileHistoryEntry {
	hash: string;
	short_hash: string;
	message: string;
	author_name: string;
	author_email: string;
	timestamp: number;
	additions: number;
	deletions: number;
}

/** Remote info for management */
export interface RemoteInfo {
	name: string;
	fetch_url: string;
	push_url: string;
}

/** Worktree info */
export interface WorktreeInfo {
	path: string;
	branch: string | null;
	head_hash: string;
	is_main: boolean;
	is_detached: boolean;
}

/** Conflict file data for 3-way merge resolution */
export interface ConflictFile {
	path: string;
	ours: string;
	theirs: string;
	base: string;
	merged: string;
}

/** Repo operation state (merge/rebase/cherry-pick in progress) */
export interface RepoOperationState {
	in_merge: boolean;
	in_rebase: boolean;
	in_cherry_pick: boolean;
	in_revert: boolean;
	conflicted_files: string[];
}

/** Rebase todo item */
export interface RebaseTodoItem {
	action: string;
	hash: string;
	short_hash: string;
	message: string;
}

/** PR review comment */
export interface ReviewComment {
	id: number;
	path: string;
	line: number | null;
	body: string;
	author: string;
	createdAt: string;
	inReplyToId: number | null;
}

/** PR changed file */
export interface PrFile {
	filename: string;
	status: string;
	additions: number;
	deletions: number;
	patch: string | null;
}

/** Git bisect state */
export interface BisectState {
	active: boolean;
	current_hash: string;
	current_short_hash: string;
	current_message: string;
	steps_remaining: number | null;
	good_hashes: string[];
	bad_hashes: string[];
	found_hash: string | null;
}

/** Active rebase state */
export interface RebaseState {
	in_progress: boolean;
	current_step: number;
	total_steps: number;
	todo_items: RebaseTodoItem[];
	head_name: string | null;
	onto: string | null;
}
