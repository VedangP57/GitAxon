//! Lane assignment algorithm for commit graph visualization.
//! GitKraken-style straight-branch forbidden-column algorithm.

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
    /// Lanes that draw a vertical pass-through line below this row.
    /// Each element is [lane_index, color_index].
    pub through_lanes: Vec<[usize; 2]>,
}

pub fn assign_lanes(commits: Vec<CommitNode>) -> Vec<LanedCommit> {
    let n = commits.len();
    if n == 0 {
        return vec![];
    }

    let commits = commits;

    // Build SHA -> index lookup
    let sha_to_idx: HashMap<String, usize> = commits
        .iter()
        .enumerate()
        .map(|(i, c)| (c.hash.clone(), i))
        .collect();

    // Build children map: sha -> Vec<child_idx>
    let mut children: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, commit) in commits.iter().enumerate() {
        for parent_hash in &commit.parent_hashes {
            children
                .entry(parent_hash.clone())
                .or_default()
                .push(i);
        }
    }

    // B: active branch list indexed by column
    // B[j] = Some(sha) means column j is occupied by that commit
    // B[j] = None means column j is free (available for reuse)
    let mut b: Vec<Option<String>> = Vec::new();

    // col_intervals: column -> list of (start_row, end_row)
    let mut col_intervals: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();

    // Lane and color result per commit
    let mut lane_result: Vec<usize> = vec![0; n];
    let mut color_result: Vec<usize> = vec![0; n];
    let mut next_color: usize = 0;

    // Process commits in row order (index 0 = newest = top)
    for i in 0..n {
        let hash = commits[i].hash.clone();
        let parent_hashes = commits[i].parent_hashes.clone();

        // Get children of this commit
        let commit_children: Vec<usize> = children
            .get(&hash)
            .cloned()
            .unwrap_or_default();

        // Branch children: children for whom this commit is their FIRST parent
        let branch_children: Vec<usize> = commit_children
            .iter()
            .filter(|&&ci| {
                commits[ci]
                    .parent_hashes
                    .first()
                    .map(|p| p == &hash)
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        // Compute forbidden columns
        let forbidden = compute_forbidden(
            i,
            &hash,
            &commits,
            &commit_children,
            &b,
            &col_intervals,
        );

        // Sort branch children by their column (leftmost first)
        let mut sorted_branch_children = branch_children.clone();
        sorted_branch_children.sort_by_key(|&ci| lane_result[ci]);

        let mut chosen_col: Option<usize> = None;
        let mut inherited_child: Option<usize> = None;

        for &ci in &sorted_branch_children {
            let child_col = lane_result[ci];
            if !forbidden.contains(&child_col) {
                if child_col < b.len() {
                    b[child_col] = Some(hash.clone());
                }
                chosen_col = Some(child_col);
                inherited_child = Some(ci);
                break;
            }
        }

        // If no column inherited, find first free slot not in forbidden
        if chosen_col.is_none() {
            let mut found = false;
            for j in 0..b.len() {
                if b[j].is_none() && !forbidden.contains(&j) {
                    b[j] = Some(hash.clone());
                    chosen_col = Some(j);
                    found = true;
                    break;
                }
            }

            if !found {
                let new_col = b.len();
                b.push(Some(hash.clone()));
                chosen_col = Some(new_col);
            }
        }

        let col = chosen_col.unwrap();
        lane_result[i] = col;
        let color = if let Some(ci) = inherited_child {
            color_result[ci]
        } else {
            let c = next_color % 8;
            next_color += 1;
            c
        };
        color_result[i] = color;

        // Clear other branch children slots (columns freed, don't extend their intervals)
        for &ci in &sorted_branch_children {
            if Some(ci) != inherited_child {
                let child_col = lane_result[ci];
                if child_col < b.len() {
                    b[child_col] = None;
                }
            }
        }

        // If this commit is a root (no parents), free its column and all lanes
        // pointing to already-processed commits (converge at repo start)
        if parent_hashes.is_empty() {
            if col < b.len() {
                b[col] = None;
            }
            let to_free: Vec<usize> = (0..b.len())
                .filter(|&j| j != col)
                .filter(|&j| {
                    b[j].as_ref()
                        .and_then(|occ| sha_to_idx.get(occ))
                        .map(|&idx| idx <= i)
                        .unwrap_or(false)
                })
                .collect();
            for j in to_free {
                b[j] = None;
            }
        }

        // Update interval for this column: extend existing or add new
        // Don't extend after column was freed (roots add final (i,i) only)
        let intervals = col_intervals.entry(col).or_default();
        if let Some(last) = intervals.last_mut() {
            if last.1 == i.saturating_sub(1) {
                last.1 = i;
            } else {
                intervals.push((i, i));
            }
        } else {
            intervals.push((i, i));
        }
    }

    // --- Sweep-line: compute through_lanes per row ---
    // through_lanes[i] = lanes active in the segment between row i and row i+1.
    // Algorithm: for each edge (commit i → parent j), lane lane_result[j] is active
    // in rows i..j (exclusive of j). Model as START at i, END at j.
    // Sweep forward: process ENDs before STARTs at each row; snapshot active set.

    let mut start_events: Vec<Vec<[usize; 2]>> = vec![vec![]; n]; // start_events[row] = vec of [lane, color]
    let mut end_events: Vec<Vec<usize>> = vec![vec![]; n];        // end_events[row] = vec of lane

    for i in 0..n {
        for parent_hash in &commits[i].parent_hashes {
            if let Some(&parent_idx) = sha_to_idx.get(parent_hash) {
                if parent_idx > i {
                    let target_lane = lane_result[parent_idx];
                    let target_color = color_result[parent_idx];
                    start_events[i].push([target_lane, target_color]);
                    end_events[parent_idx].push(target_lane);
                }
            }
        }
    }

    // active: lane → color (Vec<Option<usize>> indexed by lane for O(1) access)
    let max_lane = lane_result.iter().copied().max().unwrap_or(0);
    let mut lane_active: Vec<Option<usize>> = vec![None; max_lane + 1];
    let mut through_lanes_per_row: Vec<Vec<[usize; 2]>> = vec![vec![]; n];

    for r in 0..n {
        // END events first: lanes whose last covered row is r-1 are deactivated at r
        for &lane in &end_events[r] {
            if lane < lane_active.len() {
                lane_active[lane] = None;
            }
        }
        // START events: lanes whose coverage begins at r
        for &[lane, color] in &start_events[r] {
            if lane < lane_active.len() {
                lane_active[lane] = Some(color);
            }
        }
        // Snapshot: these lanes draw pass-through lines below row r
        through_lanes_per_row[r] = lane_active.iter().enumerate()
            .filter_map(|(l, c)| c.map(|color| [l, color]))
            .collect();
    }

    // Build LanedCommit with edges (parents have lanes assigned)
    let mut result = Vec::with_capacity(n);
    for (i, tl) in through_lanes_per_row.into_iter().enumerate() {
        let edges = generate_edges(i, &commits, &sha_to_idx, &lane_result, &color_result);
        result.push(LanedCommit {
            commit: commits[i].clone(),
            lane: lane_result[i],
            color_index: color_result[i],
            edges,
            through_lanes: tl,
        });
    }

    result
}

fn compute_forbidden(
    row: usize,
    hash: &str,
    commits: &[CommitNode],
    commit_children: &[usize],
    b: &[Option<String>],
    col_intervals: &HashMap<usize, Vec<(usize, usize)>>,
) -> HashSet<usize> {
    // Merge children: children for whom this commit is NOT the first parent
    let merge_children: Vec<usize> = commit_children
        .iter()
        .filter(|&&ci| {
            commits[ci]
                .parent_hashes
                .first()
                .map(|p| p != hash)
                .unwrap_or(true)
        })
        .copied()
        .collect();

    if merge_children.is_empty() {
        return HashSet::new();
    }

    let i_min = *merge_children.iter().min().unwrap_or(&row);

    let mut forbidden = HashSet::new();

    for (j, occupant) in b.iter().enumerate() {
        if occupant.is_some() {
            if column_occupied_between(j, i_min, row, col_intervals) {
                forbidden.insert(j);
            }
        }
    }

    forbidden
}

fn column_occupied_between(
    col: usize,
    row_start: usize,
    row_end: usize,
    col_intervals: &HashMap<usize, Vec<(usize, usize)>>,
) -> bool {
    if let Some(intervals) = col_intervals.get(&col) {
        for &(a, b) in intervals {
            if a <= row_end && b >= row_start {
                return true;
            }
        }
    }
    false
}

fn generate_edges(
    row: usize,
    commits: &[CommitNode],
    sha_to_idx: &HashMap<String, usize>,
    lane_result: &[usize],
    color_result: &[usize],
) -> Vec<Edge> {
    let parent_hashes = &commits[row].parent_hashes;
    let my_col = lane_result[row];
    let my_color = color_result[row];

    let mut edges = Vec::new();

    for (pi, parent_hash) in parent_hashes.iter().enumerate() {
        if let Some(&parent_idx) = sha_to_idx.get(parent_hash) {
            let parent_col = lane_result[parent_idx];
            let parent_color = color_result[parent_idx];

            if pi == 0 {
                // First parent: straight line or fork
                let edge_type = if my_col == parent_col {
                    EdgeType::Straight
                } else {
                    EdgeType::Fork
                };
                edges.push(Edge {
                    from_lane: my_col,
                    to_lane: parent_col,
                    edge_type,
                    color_index: my_color,
                });
            } else {
                // Additional parents: merge edge
                edges.push(Edge {
                    from_lane: my_col,
                    to_lane: parent_col,
                    edge_type: EdgeType::Merge,
                    color_index: parent_color,
                });
            }
        }
    }

    edges
}
