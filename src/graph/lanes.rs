//! Lane assignment algorithm for commit graph visualization.
//! GitKraken-style "Straight Branches" algorithm from academic research.

use std::collections::{HashMap, HashSet};

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

/// Active slot: (commit_hash, color_index). None means free, never shift slots.
type ActiveSlot = Option<(String, usize)>;

pub fn assign_lanes(commits: Vec<CommitNode>) -> Vec<LanedCommit> {
    if commits.is_empty() {
        return vec![];
    }

    let n = commits.len();

    // Map hash -> index in commits array
    let hash_to_idx: HashMap<String, usize> = commits
        .iter()
        .enumerate()
        .map(|(i, c)| (c.hash.clone(), i))
        .collect();

    // Build children map: hash -> [child_hash, ...]
    let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
    for c in &commits {
        for parent in &c.parent_hashes {
            children_map
                .entry(parent.clone())
                .or_default()
                .push(c.hash.clone());
        }
    }

    // For each commit: branch_children = children where this IS their first parent
    let branch_children: HashMap<String, Vec<String>> = commits
        .iter()
        .map(|c| {
            let bc: Vec<String> = children_map
                .get(&c.hash)
                .unwrap_or(&vec![])
                .iter()
                .filter(|child_hash| {
                    hash_to_idx
                        .get(child_hash.as_str())
                        .and_then(|&child_idx| {
                            commits[child_idx].parent_hashes.first().map(|p| p == &c.hash)
                        })
                        .unwrap_or(false)
                })
                .cloned()
                .collect();
            (c.hash.clone(), bc)
        })
        .collect();

    // merge_children = children where this is NOT their first parent
    let merge_children: HashMap<String, Vec<String>> = commits
        .iter()
        .map(|c| {
            let mc: Vec<String> = children_map
                .get(&c.hash)
                .unwrap_or(&vec![])
                .iter()
                .filter(|child_hash| {
                    hash_to_idx
                        .get(child_hash.as_str())
                        .and_then(|&child_idx| {
                            commits[child_idx].parent_hashes.first().map(|p| p != &c.hash)
                        })
                        .unwrap_or(false)
                })
                .cloned()
                .collect();
            (c.hash.clone(), mc)
        })
        .collect();

    // Active branches list — slots that are None can be reused, never shift
    let mut active: Vec<ActiveSlot> = Vec::new();

    // Track which column each commit hash is in
    let mut hash_to_lane: HashMap<String, usize> = HashMap::new();

    // Color counter
    let mut color_counter: usize = 0;

    // Track occupied column ranges for forbidden column computation
    let mut col_last_used: Vec<i64> = Vec::new();

    let mut lane_result: Vec<usize> = vec![0; n];
    let mut color_result: Vec<usize> = vec![0; n];

    // Process commits from newest (index 0) to oldest. We need children's lanes when
    // processing a parent, so input must be newest-first (enforced in get_commits).
    for i in 0..n {
        let c = &commits[i];
        let row = i;

        // Compute forbidden columns: columns with active edges between
        // this commit and its earliest merge child
        let my_merge_children = merge_children
            .get(&c.hash)
            .cloned()
            .unwrap_or_default();

        let forbidden: HashSet<usize> = if my_merge_children.is_empty() {
            HashSet::new()
        } else {
            let earliest_merge_child_row = my_merge_children
                .iter()
                .filter_map(|h| hash_to_idx.get(h).copied())
                .min()
                .unwrap_or(row);

            col_last_used
                .iter()
                .enumerate()
                .filter(|(_, &last)| last >= earliest_merge_child_row as i64)
                .map(|(col, _)| col)
                .collect()
        };

        let my_branch_children = branch_children
            .get(&c.hash)
            .cloned()
            .unwrap_or_default();

        // Find a branch child whose lane is not forbidden
        let inherit_child = my_branch_children.iter().find_map(|h| {
            hash_to_lane.get(h).copied().filter(|&col| !forbidden.contains(&col))
        });

        let my_lane: usize;
        let my_color: usize;

        if let Some(col) = inherit_child {
            // Inherit lane from branch child (continue straight line)
            my_lane = col;
            my_color = active
                .get(col)
                .and_then(|s| s.as_ref())
                .map(|(_, c)| *c)
                .unwrap_or_else(|| {
                    let c = color_counter % 8;
                    color_counter += 1;
                    c
                });
            if col < active.len() {
                active[col] = Some((c.hash.clone(), my_color));
            } else {
                while active.len() <= col {
                    active.push(None);
                }
                active[col] = Some((c.hash.clone(), my_color));
            }
        } else {
            // Find a free slot not in forbidden
            let free_slot = active
                .iter()
                .enumerate()
                .find(|(col, slot)| slot.is_none() && !forbidden.contains(col))
                .map(|(col, _)| col);

            my_color = color_counter % 8;
            color_counter += 1;

            if let Some(col) = free_slot {
                my_lane = col;
                active[col] = Some((c.hash.clone(), my_color));
            } else {
                // Append new lane
                my_lane = active.len();
                active.push(Some((c.hash.clone(), my_color)));
                col_last_used.push(-1);
            }
        }

        lane_result[i] = my_lane;
        color_result[i] = my_color;
        hash_to_lane.insert(c.hash.clone(), my_lane);

        // Update col_last_used for this lane
        while col_last_used.len() <= my_lane {
            col_last_used.push(-1);
        }
        col_last_used[my_lane] = row as i64;

        // Free slots for branch children that we're not inheriting from
        for child_hash in &my_branch_children {
            if let Some(&child_col) = hash_to_lane.get(child_hash) {
                if child_col != my_lane && child_col < active.len() {
                    active[child_col] = None;
                }
            }
        }
    }

    // Build LanedCommit with edges
    let mut result = Vec::with_capacity(n);

    for i in 0..n {
        let c = &commits[i];
        let my_lane = lane_result[i];
        let my_color = color_result[i];

        let mut edges = Vec::new();

        for (p_idx, parent_hash) in c.parent_hashes.iter().enumerate() {
            if let Some(&parent_idx) = hash_to_idx.get(parent_hash) {
                let parent_lane = lane_result[parent_idx];
                let parent_color = color_result[parent_idx];

                let edge_type = if p_idx == 0 && my_lane == parent_lane {
                    EdgeType::Straight
                } else if p_idx == 0 {
                    EdgeType::Fork
                } else {
                    EdgeType::Merge
                };

                let color = if p_idx == 0 { my_color } else { parent_color };

                edges.push(Edge {
                    from_lane: my_lane,
                    to_lane: parent_lane,
                    color_index: color,
                    edge_type,
                });
            }
        }

        result.push(LanedCommit {
            commit: c.clone(),
            lane: my_lane,
            color_index: my_color,
            edges,
        });
    }

    result
}
