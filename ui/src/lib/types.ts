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

export interface RepoRecord {
	id: string;
	path: string;
	name: string;
	last_opened: number;
	is_favorite: boolean;
}
