//! Block derivation: signals cut the track net into blocks.
//!
//! A block is a connected set of tracks; two stubs belong to the same block
//! if they are reachable from each other without passing a node where a
//! signal anchors. Both travel directions of a stub always share a block —
//! Factorio semantics: a signal splits the block regardless of direction.

use crate::graph::EdgeData;
use crate::units::{BlockId, EdgeId, NodeId};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct BlockSet {
    /// Block of each directed edge (opposite edges always agree).
    pub edge_block: Vec<BlockId>,
    pub count: u32,
}

impl BlockSet {
    pub fn block_of(&self, edge: EdgeId) -> BlockId {
        self.edge_block[edge.0 as usize]
    }
}

const UNASSIGNED: BlockId = BlockId(u32::MAX);

/// Flood fill over undirected stubs. `cut_nodes` are connector points with
/// at least one signal — traversal never crosses them. Deterministic: block
/// ids are assigned in ascending order of the first edge encountered.
pub(crate) fn derive(edges: &[EdgeData], cut_nodes: &BTreeSet<NodeId>) -> BlockSet {
    let canonical = |i: u32| -> u32 { i.min(edges[i as usize].opposite.0) };

    // Node -> incident canonical (undirected) units.
    let mut incident: BTreeMap<NodeId, Vec<u32>> = BTreeMap::new();
    for (i, edge) in edges.iter().enumerate() {
        let i = i as u32;
        if i != canonical(i) {
            continue;
        }
        incident.entry(edge.from).or_default().push(i);
        incident.entry(edge.to).or_default().push(i);
    }

    let mut edge_block = vec![UNASSIGNED; edges.len()];
    let mut count = 0u32;
    for start in 0..edges.len() as u32 {
        if start != canonical(start) || edge_block[start as usize] != UNASSIGNED {
            continue;
        }
        let block = BlockId(count);
        count += 1;
        let mut stack = vec![start];
        while let Some(unit) = stack.pop() {
            if edge_block[unit as usize] != UNASSIGNED {
                continue;
            }
            edge_block[unit as usize] = block;
            edge_block[edges[unit as usize].opposite.0 as usize] = block;
            let edge = &edges[unit as usize];
            for node in [edge.from, edge.to] {
                if cut_nodes.contains(&node) {
                    continue;
                }
                for &neighbor in &incident[&node] {
                    if edge_block[neighbor as usize] == UNASSIGNED {
                        stack.push(neighbor);
                    }
                }
            }
        }
    }

    BlockSet { edge_block, count }
}
