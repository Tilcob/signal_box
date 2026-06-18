//! The derived runtime graph: nodes (connector points + cell centers),
//! directed stub edges with precomputed continuations, signal anchors, and
//! the block partition. Built once at sim start — cells are irrelevant
//! afterward.
//!
//! Routing never needs geometry at runtime: every directed edge knows its
//! continuation (`Next`), switches are the only dynamic decision points.
//!
//! Known M0 limitation (deliberate): two routes crossing in one cell share
//! the center node, and therefore the block — block signals are the
//! protection. A geometric collision *at* the crossing point between trains
//! on different stubs is not separately detected.

use crate::blocks::{self, BlockSet};
use crate::grid::{Cell, Point};
use crate::layout::{self, Layout, SignalKind, SwitchRule, ValidationError};
use crate::level::Level;
use crate::units::{EdgeId, Len, NodeId, SignalId, SinkId, SourceId};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// Connector point (edge midpoint/corner) — shared between cells.
    Connector,
    /// Cell center; `switch` indexes [`TrackGraph::switches`] if this cell
    /// is a switch.
    Center { switch: Option<u32> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeData {
    pub point: Point,
    pub kind: NodeKind,
}

/// Where a train continues after traversing a directed edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    Fixed(EdgeId),
    /// Arriving at a switch center via the stem: the switch configuration
    /// decides (index into [`TrackGraph::switches`]).
    SwitchChoice {
        switch: u32,
    },
    DeadEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeData {
    pub from: NodeId,
    pub to: NodeId,
    pub len: Len,
    pub opposite: EdgeId,
    pub next: Next,
    /// Signal gating this edge's endpoint (`to` is the stop position).
    pub signal: Option<SignalId>,
}

/// A switch, resolved to graph terms. Rules are copied from the layout so
/// the graph is self-contained.
#[derive(Debug, Clone)]
pub struct SwitchData {
    pub cell: Cell,
    pub center: NodeId,
    /// Outgoing edge center → stem connector (used by trailing moves).
    pub stem_out: EdgeId,
    /// Outgoing edges center → branch connector, indexed like the layout.
    pub branch_out: [EdgeId; 2],
    pub default_branch: u8,
    pub rules: Vec<SwitchRule>,
}

#[derive(Debug, Clone, Copy)]
pub struct SignalData {
    pub kind: SignalKind,
    /// The directed edge this signal gates (its `to` is the stop point).
    pub edge: EdgeId,
    /// Block-contention priority (higher wins); copied from the layout.
    pub priority: i8,
}

#[derive(Debug, Clone, Copy)]
pub struct SourceData {
    pub id: SourceId,
    /// Entry edge: connector point → cell center of the source cell.
    pub entry: EdgeId,
}

#[derive(Debug, Clone, Copy)]
pub struct SinkData {
    pub id: SinkId,
    /// Arrival edge: cell center → connector point; reaching its end is
    /// the arrival moment.
    pub arrival: EdgeId,
}

#[derive(Debug, Clone)]
pub struct TrackGraph {
    pub nodes: Vec<NodeData>,
    pub edges: Vec<EdgeData>,
    pub switches: Vec<SwitchData>,
    pub signals: Vec<SignalData>,
    pub blocks: BlockSet,
    pub sources: Vec<SourceData>,
    pub sinks: Vec<SinkData>,
}

impl TrackGraph {
    pub fn edge(&self, id: EdgeId) -> &EdgeData {
        &self.edges[id.0 as usize]
    }

    pub fn node(&self, id: NodeId) -> &NodeData {
        &self.nodes[id.0 as usize]
    }
}

fn push_stub(edges: &mut Vec<EdgeData>, p: NodeId, c: NodeId, len: Len) -> (usize, usize) {
    let into = edges.len();
    let out = into + 1;
    edges.push(EdgeData {
        from: p,
        to: c,
        len,
        opposite: EdgeId(out as u32),
        next: Next::DeadEnd, // placeholder, filled below
        signal: None,
    });
    edges.push(EdgeData {
        from: c,
        to: p,
        len,
        opposite: EdgeId(into as u32),
        next: Next::DeadEnd, // placeholder, filled below
        signal: None,
    });
    (into, out)
}

/// Validates and builds. `Err` carries the complete validation report.
pub fn build(level: &Level, player: &Layout) -> Result<TrackGraph, Vec<ValidationError>> {
    let errors = layout::validate(level, player);
    if !errors.is_empty() {
        return Err(errors);
    }
    let merged = level.fixed.merged(player);

    // Deterministic processing order for everything that mints ids.
    let mut pieces = merged.pieces.clone();
    for piece in &mut pieces {
        if piece.a.index() > piece.b.index() {
            std::mem::swap(&mut piece.a, &mut piece.b);
        }
    }
    pieces.sort_by_key(|p| (p.cell, p.a, p.b));
    let mut switches = merged.switches.clone();
    switches.sort_by_key(|s| s.cell);
    let mut signals = merged.signals.clone();
    signals.sort_by_key(|s| (s.cell, s.at));

    // 1. Nodes, ids in lattice-point order.
    let mut kinds: BTreeMap<Point, NodeKind> = BTreeMap::new();
    for piece in &pieces {
        kinds.insert(piece.cell.center_point(), NodeKind::Center { switch: None });
        for dir in [piece.a, piece.b] {
            kinds
                .entry(piece.cell.connector_point(dir))
                .or_insert(NodeKind::Connector);
        }
    }
    for (i, switch) in switches.iter().enumerate() {
        kinds.insert(
            switch.cell.center_point(),
            NodeKind::Center {
                switch: Some(i as u32),
            },
        );
        for dir in [switch.stem, switch.branches[0], switch.branches[1]] {
            kinds
                .entry(switch.cell.connector_point(dir))
                .or_insert(NodeKind::Connector);
        }
    }
    let mut node_id: BTreeMap<Point, NodeId> = BTreeMap::new();
    let mut nodes: Vec<NodeData> = Vec::new();
    for (point, kind) in &kinds {
        node_id.insert(*point, NodeId(nodes.len() as u32));
        nodes.push(NodeData {
            point: *point,
            kind: *kind,
        });
    }

    // 2. Edges: two stubs per piece, three per switch; in-cell continuations.
    let mut edges: Vec<EdgeData> = Vec::new();
    for piece in &pieces {
        let center = node_id[&piece.cell.center_point()];
        let pa = node_id[&piece.cell.connector_point(piece.a)];
        let pb = node_id[&piece.cell.connector_point(piece.b)];
        let (a_in, a_out) = push_stub(&mut edges, pa, center, piece.a.half_len());
        let (b_in, b_out) = push_stub(&mut edges, pb, center, piece.b.half_len());
        edges[a_in].next = Next::Fixed(EdgeId(b_out as u32));
        edges[b_in].next = Next::Fixed(EdgeId(a_out as u32));
    }
    let mut switch_data: Vec<SwitchData> = Vec::new();
    for (i, switch) in switches.iter().enumerate() {
        let center = node_id[&switch.cell.center_point()];
        let p_stem = node_id[&switch.cell.connector_point(switch.stem)];
        let p_b0 = node_id[&switch.cell.connector_point(switch.branches[0])];
        let p_b1 = node_id[&switch.cell.connector_point(switch.branches[1])];
        let (stem_in, stem_out) = push_stub(&mut edges, p_stem, center, switch.stem.half_len());
        let (b0_in, b0_out) = push_stub(&mut edges, p_b0, center, switch.branches[0].half_len());
        let (b1_in, b1_out) = push_stub(&mut edges, p_b1, center, switch.branches[1].half_len());
        edges[stem_in].next = Next::SwitchChoice { switch: i as u32 };
        // Trailing moves (arriving via a branch) always continue to the stem.
        edges[b0_in].next = Next::Fixed(EdgeId(stem_out as u32));
        edges[b1_in].next = Next::Fixed(EdgeId(stem_out as u32));
        switch_data.push(SwitchData {
            cell: switch.cell,
            center,
            stem_out: EdgeId(stem_out as u32),
            branch_out: [EdgeId(b0_out as u32), EdgeId(b1_out as u32)],
            default_branch: switch.default_branch,
            rules: switch.rules.clone(),
        });
    }

    // 3. Continuations across connector points: the unique other stub there
    //    (uniqueness guaranteed by the junction + connector-reuse rules).
    let mut leaving: BTreeMap<NodeId, Vec<u32>> = BTreeMap::new();
    for (i, edge) in edges.iter().enumerate() {
        if matches!(nodes[edge.from.0 as usize].kind, NodeKind::Connector) {
            leaving.entry(edge.from).or_default().push(i as u32);
        }
    }
    let crossings: Vec<(usize, Next)> = edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| matches!(nodes[edge.to.0 as usize].kind, NodeKind::Connector))
        .map(|(i, edge)| {
            let mut next = Next::DeadEnd;
            if let Some(candidates) = leaving.get(&edge.to) {
                for &candidate in candidates {
                    if candidate != edge.opposite.0 {
                        next = Next::Fixed(EdgeId(candidate));
                    }
                }
            }
            (i, next)
        })
        .collect();
    for (i, next) in crossings {
        edges[i].next = next;
    }

    // 4. Signals: each gates the edge cell-center → anchored connector.
    let mut edge_by_pair: BTreeMap<(NodeId, NodeId), u32> = BTreeMap::new();
    for (i, edge) in edges.iter().enumerate() {
        edge_by_pair.insert((edge.from, edge.to), i as u32);
    }
    let mut signal_data: Vec<SignalData> = Vec::new();
    let mut cut_nodes: BTreeSet<NodeId> = BTreeSet::new();
    for signal in &signals {
        let center = node_id[&signal.cell.center_point()];
        let point = node_id[&signal.cell.connector_point(signal.at)];
        let gated = edge_by_pair[&(center, point)];
        edges[gated as usize].signal = Some(SignalId(signal_data.len() as u32));
        signal_data.push(SignalData {
            kind: signal.kind,
            edge: EdgeId(gated),
            priority: signal.priority,
        });
        cut_nodes.insert(point);
    }

    // 5. Blocks.
    let blocks = blocks::derive(&edges, &cut_nodes);

    // 6. Sources and sinks (anchoring validated).
    let sources = level
        .sources
        .iter()
        .map(|source| SourceData {
            id: source.id,
            entry: EdgeId(
                edge_by_pair[&(
                    node_id[&source.cell.connector_point(source.dir)],
                    node_id[&source.cell.center_point()],
                )],
            ),
        })
        .collect();
    let sinks = level
        .sinks
        .iter()
        .map(|sink| SinkData {
            id: sink.id,
            arrival: EdgeId(
                edge_by_pair[&(
                    node_id[&sink.cell.center_point()],
                    node_id[&sink.cell.connector_point(sink.dir)],
                )],
            ),
        })
        .collect();

    Ok(TrackGraph {
        nodes,
        edges,
        switches: switch_data,
        signals: signal_data,
        blocks,
        sources,
        sinks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Dir8;
    use crate::layout::{SignalDef, SwitchDef, TrackPiece};
    use crate::level::{Par, ScheduleEntry, SinkDef, SourceDef};
    use crate::units::segment_lengths::STRAIGHT;
    use crate::units::{Speed, Tick, TrainClass, TrainId};

    fn cell(x: i32, y: i32) -> Cell {
        Cell { x, y }
    }

    fn straight_level(n: i32) -> (Level, Layout) {
        let layout = Layout {
            pieces: (0..n)
                .map(|x| TrackPiece {
                    cell: cell(x, 0),
                    a: Dir8::W,
                    b: Dir8::E,
                })
                .collect(),
            switches: vec![],
            signals: vec![],
        };
        let level = Level {
            name: "test".into(),
            buildable: (0..n).map(|x| cell(x, 0)).collect(),
            fixed: Layout::default(),
            sources: vec![SourceDef {
                id: SourceId(0),
                cell: cell(0, 0),
                dir: Dir8::W,
                label: String::new(),
            }],
            sinks: vec![SinkDef {
                id: SinkId(0),
                cell: cell(n - 1, 0),
                dir: Dir8::E,
                label: "OST".into(),
            }],
            schedule: vec![ScheduleEntry {
                train: TrainId(0),
                class: TrainClass(0),
                length: Len(800),
                speed: Speed(100),
                source: SourceId(0),
                sink: SinkId(0),
                depart: Tick(0),
                due: Tick(200),
            }],
            par: Par {
                throughput: Tick(200),
                material: n as u32,
                lateness: 0,
            },
        };
        (level, layout)
    }

    /// Follows `Next::Fixed` from an edge to the dead end, summing lengths.
    fn walk(graph: &TrackGraph, start: EdgeId) -> (Len, usize) {
        let mut len = Len(0);
        let mut hops = 0;
        let mut current = start;
        loop {
            let edge = graph.edge(current);
            len = len + edge.len;
            hops += 1;
            assert!(hops < 1000, "walk does not terminate");
            match edge.next {
                Next::Fixed(next) => current = next,
                Next::DeadEnd => return (len, hops),
                Next::SwitchChoice { .. } => panic!("unexpected switch"),
            }
        }
    }

    #[test]
    fn straight_line_counts_and_walk() {
        let (level, layout) = straight_level(3);
        let graph = build(&level, &layout).expect("valid");
        // 3 centers + 4 connector points; 3 pieces × 2 stubs × 2 directions.
        assert_eq!(graph.nodes.len(), 7);
        assert_eq!(graph.edges.len(), 12);
        assert_eq!(graph.blocks.count, 1);

        let entry = graph.sources[0].entry;
        let (len, hops) = walk(&graph, entry);
        assert_eq!(len, Len(STRAIGHT.0 * 3));
        assert_eq!(hops, 6);
        // The walk ends on the sink's arrival edge.
        assert_eq!(
            graph.edges[graph.sinks[0].arrival.0 as usize].next,
            Next::DeadEnd
        );
    }

    #[test]
    fn signal_cuts_blocks_and_gates_the_right_edge() {
        let (level, mut layout) = straight_level(3);
        layout.signals.push(SignalDef {
            cell: cell(1, 0),
            at: Dir8::E,
            kind: SignalKind::Block,
            priority: 0,
        });
        let graph = build(&level, &layout).expect("valid");
        assert_eq!(graph.blocks.count, 2);

        let gated = graph.edge(graph.signals[0].edge);
        // Gated edge runs center(1,0) → connector point E of (1,0).
        assert_eq!(graph.node(gated.from).point, cell(1, 0).center_point());
        assert_eq!(
            graph.node(gated.to).point,
            cell(1, 0).connector_point(Dir8::E)
        );
        assert_eq!(gated.signal, Some(SignalId(0)));
        // The opposite direction is not gated.
        assert_eq!(graph.edge(gated.opposite).signal, None);
    }

    #[test]
    fn switch_routing_hooks() {
        let (mut level, mut layout) = straight_level(1);
        level.buildable.extend([cell(1, 0), cell(2, 0), cell(2, 1)]);
        level.sinks.push(SinkDef {
            id: SinkId(1),
            cell: cell(2, 1),
            dir: Dir8::NE,
            label: "NORD".into(),
        });
        // sink 0 moves to the end of the straight branch.
        level.sinks[0].cell = cell(2, 0);
        layout.switches.push(SwitchDef {
            cell: cell(1, 0),
            stem: Dir8::W,
            branches: [Dir8::E, Dir8::NE],
            default_branch: 0,
            rules: vec![],
        });
        layout.pieces.push(TrackPiece {
            cell: cell(2, 0),
            a: Dir8::W,
            b: Dir8::E,
        });
        layout.pieces.push(TrackPiece {
            cell: cell(2, 1),
            a: Dir8::SW,
            b: Dir8::NE,
        });
        let graph = build(&level, &layout).expect("valid");
        assert_eq!(graph.switches.len(), 1);
        let switch = &graph.switches[0];

        // Walk from the source: entry stub, out of cell (0,0), into the
        // switch stem — which must end in a SwitchChoice.
        let entry = graph.sources[0].entry;
        let mut current = entry;
        let stem_in = loop {
            match graph.edge(current).next {
                Next::Fixed(next) => current = next,
                Next::SwitchChoice { switch: s } => {
                    assert_eq!(s, 0);
                    break current;
                }
                Next::DeadEnd => panic!("hit dead end before switch"),
            }
        };
        assert_eq!(graph.edge(stem_in).to, switch.center);

        // Trailing move: coming back from a branch continues via the stem.
        let from_branch = graph.edge(switch.branch_out[0]).opposite;
        assert_eq!(graph.edge(from_branch).next, Next::Fixed(switch.stem_out));

        // Both branches reach their sinks.
        let (_, hops0) = walk(&graph, switch.branch_out[0]);
        assert!(hops0 >= 3);
        let (_, hops1) = walk(&graph, switch.branch_out[1]);
        assert!(hops1 >= 3);
        assert_eq!(graph.blocks.count, 1);
    }
}
