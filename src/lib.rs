//! Gitfast-core: fast git operations library.

pub mod bisect;
pub mod branches;
pub mod cache;
pub mod conflicts;
pub mod identity;
pub mod diff;
pub mod errors;
pub mod graph;
pub mod history;
pub mod rebase;
pub mod remotes;
pub mod repo_pool;
pub mod staging;
pub mod stash;
pub mod tags;
pub mod watcher;
pub mod worktree;


// Types - cache
pub use cache::{CommitNode, RepoRecord};

// Types - errors
pub use errors::{GitfastError, GitfastResult};

// Types - graph
pub use graph::{assign_lanes, Edge, EdgeType, LanedCommit};

// Types - diff
pub use diff::{BlameLine, DiffFile, DiffHunk, DiffLine, FileStatus, LineType};

// Types - staging
pub use staging::{IndexEntry, StagingStatus};

// Types - branches
pub use branches::BranchInfo;

// Types - tags
pub use tags::TagInfo;

// Types - remotes
pub use remotes::{FetchResult, PushResult, RemoteInfo};

// JSON functions - graph
pub use graph::{get_commits_json, get_laned_commits_json};

// JSON functions - diff
pub use diff::{diff_commit_json, diff_staged_json, diff_working_tree_json};

// JSON functions - staging
pub use staging::get_status_json;

// JSON functions - branches
pub use branches::list_branches_json;

// JSON functions - tags
pub use tags::list_tags_json;

// JSON functions - remotes
pub use remotes::list_remotes_json;
