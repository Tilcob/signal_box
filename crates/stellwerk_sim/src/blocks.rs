//! Block derivation: signals cut the track net into blocks.
//!
//! A block is a set of track a train can *travel within* without passing a
//! signal — connectivity follows the continuation (`next`) relation, NOT raw
//! node incidence. The difference is the flat crossing: its two routes share
//! the centre node geometrically but no train transfers between them, so they
//! are SEPARATE blocks (they conflict only at the crossing point, handled by
//! the sim's node check). A switch never splits a block (stem ↔ both branches),
//! so switch centres connect all their stubs. Both travel directions of a stub
//! always share a block — Factorio semantics: a signal splits regardless of
//! direction.

use crate::graph::{EdgeData, Next};
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

/// Flood fill over stubs, connected by the continuation relation. `cut_nodes`
/// are connector points with at least one signal — traversal never crosses
/// them. `switch_centers` connect ALL their incident stubs (a switch never
/// splits a block); every other node connects only continuation-paired stubs,
/// which keeps a flat crossing's two routes apart. Deterministic: block ids are
/// assigned in ascending order of the first edge encountered.
pub(crate) fn derive(
    edges: &[EdgeData],
    cut_nodes: &BTreeSet<NodeId>,
    switch_centers: &BTreeSet<NodeId>,
) -> BlockSet {
    let canonical = |i: u32| -> u32 { i.min(edges[i as usize].opposite.0) };

    // Node -> incident canonical (undirected) units (used at switch centres).
    let mut incident: BTreeMap<NodeId, Vec<u32>> = BTreeMap::new();
    for (i, edge) in edges.iter().enumerate() {
        let i = i as u32;
        if i != canonical(i) {
            continue;
        }
        incident.entry(edge.from).or_default().push(i);
        incident.entry(edge.to).or_default().push(i);
    }

    // The block-mate of canonical stub `u` at node `b`: the stub a train flows
    // to/from across `b`. Take the directed edge of `u` pointing INTO `b`; its
    // `Fixed` continuation's canonical stub is the partner. `None` at a dead end
    // or a switch choice (switch centres use `incident` instead).
    let partner_at = |u: u32, b: NodeId| -> Option<u32> {
        let into = if edges[u as usize].to == b {
            u
        } else {
            edges[u as usize].opposite.0
        };
        match edges[into as usize].next {
            Next::Fixed(n) => Some(canonical(n.0)),
            _ => None,
        }
    };

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
                if switch_centers.contains(&node) {
                    for &neighbor in &incident[&node] {
                        if edge_block[neighbor as usize] == UNASSIGNED {
                            stack.push(neighbor);
                        }
                    }
                } else if let Some(partner) = partner_at(unit, node)
                    && edge_block[partner as usize] == UNASSIGNED
                {
                    stack.push(partner);
                }
            }
        }
    }

    BlockSet { edge_block, count }
}
