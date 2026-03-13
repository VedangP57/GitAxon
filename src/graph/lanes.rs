//! Lane assignment algorithm for commit graph visualization.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::cache::CommitNode;

/// Type of edge between commits in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum EdgeType {
    /// Straight vertical line in the same lane.
    Straight,
    /// Branch point: one lane splits into multiple.
    Fork,
    /// Merge: multiple lanes join into one.
    Merge,
}

/// Edge describing the connection from one commit to a parent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// Lane of the source commit (this commit).
    pub from_lane: usize,
    /// Lane of the target commit (parent).
    pub to_lane: usize,
    /// Color index for the edge (0-7).
    pub color_index: usize,
    /// Type of edge for rendering.
    pub edge_type: EdgeType,
}

/// Commit with lane assignment for graph rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanedCommit {
    /// The commit data.
    pub commit: CommitNode,
    /// Assigned lane (column) for this commit.
    pub lane: usize,
    /// Color index for this commit's line (0-7).
    pub color_index: usize,
    /// Edges to parent commits.
    pub edges: Vec<Edge>,
}

/// Assigns lanes to commits for visual graph rendering.
///
/// Commits must be in chronological order (newest first, as returned by git log).
pub fn assign_lanes(commits: Vec<CommitNode>) -> Vec<LanedCommit> {
    if commits.is_empty() {
        return Vec::new();
    }

    let n = commits.len();
    let hash_to_idx: HashMap<&str, usize> = commits
        .iter()
        .enumerate()
        .map(|(i, c)| (c.hash.as_str(), i))
        .collect();

    // active_lanes: commit_hash -> lane (set by children when they process their parents)
    let mut active_lanes: HashMap<String, usize> = HashMap::new();
    let mut next_lane: usize = 0;
    let mut lane_of: Vec<usize> = vec![0; n];
    let mut color_of_lane: HashMap<usize, usize> = HashMap::new();

    for i in 0..n {
        let commit = &commits[i];
        let h = commit.hash.as_str();

        // Use lane from active_lanes if set by a child, otherwise assign new
        let lane = *active_lanes.entry(h.to_string()).or_insert_with(|| {
            let lane = next_lane;
            next_lane += 1;
            lane
        });
        lane_of[i] = lane;
        color_of_lane.entry(lane).or_insert_with(|| lane % 8);

        // Assign lanes to parents: first parent gets our lane, others get new lanes
        // Use or_insert / or_insert_with so we don't overwrite if already set by another child
        for (parent_idx, parent_hash) in commit.parent_hashes.iter().enumerate() {
            if hash_to_idx.contains_key(parent_hash.as_str()) {
                if parent_idx == 0 {
                    active_lanes.entry(parent_hash.clone()).or_insert(lane);
                } else {
                    active_lanes.entry(parent_hash.clone()).or_insert_with(|| {
                        let l = next_lane;
                        next_lane += 1;
                        l
                    });
                }
            }
        }
    }

    // Build edges (pass 2): for each commit, edges to its parents
    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        let commit = &commits[i];
        let lane = lane_of[i];
        let color_index = color_of_lane.get(&lane).copied().unwrap_or(lane % 8);
        let is_merge = commit.parent_hashes.len() > 1;

        let mut edges = Vec::new();
        for (parent_idx, parent_hash) in commit.parent_hashes.iter().enumerate() {
            if let Some(&parent_i) = hash_to_idx.get(parent_hash.as_str()) {
                let to_lane = lane_of[parent_i];
                let parent_color = color_of_lane.get(&to_lane).copied().unwrap_or(to_lane % 8);
                let edge_type = if lane == to_lane {
                    EdgeType::Straight
                } else if is_merge && parent_idx > 0 {
                    EdgeType::Merge
                } else {
                    EdgeType::Fork
                };
                edges.push(Edge {
                    from_lane: lane,
                    to_lane,
                    color_index: parent_color,
                    edge_type,
                });
            }
        }

        result.push(LanedCommit {
            commit: commit.clone(),
            lane,
            color_index,
            edges,
        });
    }

    result
}
