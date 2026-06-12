//! The track network as a plain graph resource: nodes (with positions),
//! undirected edges, and switches that pick one of several continuations.
//! Trains, rendering and interaction all read (and toggle) this one resource.

use bevy::prelude::*;

/// What a train carries — and which station it must reach.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cargo {
    Orange,
    Blue,
}

impl Cargo {
    pub fn color(self) -> Color {
        match self {
            Cargo::Orange => Color::srgb(0.95, 0.60, 0.15),
            Cargo::Blue => Color::srgb(0.30, 0.62, 0.95),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Cargo::Orange => "ORANGE",
            Cargo::Blue => "BLAU",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum NodeKind {
    /// Plain junction/bend — trains pass through.
    Plain,
    /// Trains enter the world here. The label is shown next to the node.
    Source(&'static str),
    /// Terminus. Trains despawn here; cargo must match to count as delivered.
    Sink(Cargo),
}

pub struct Node {
    pub pos: Vec2,
    pub kind: NodeKind,
}

pub struct Edge {
    pub a: usize,
    pub b: usize,
    pub len: f32,
}

/// A facing-point switch: a train passing `node` continues toward
/// `options[selected]`. Toggled by clicking near the node.
pub struct Switch {
    pub node: usize,
    pub options: [usize; 2],
    pub selected: usize,
}

#[derive(Resource)]
pub struct TrackGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub switches: Vec<Switch>,
}

impl TrackGraph {
    pub fn pos(&self, node: usize) -> Vec2 {
        self.nodes[node].pos
    }

    pub fn edge_len(&self, a: usize, b: usize) -> f32 {
        self.edges
            .iter()
            .find(|e| (e.a == a && e.b == b) || (e.a == b && e.b == a))
            .map(|e| e.len)
            .unwrap_or_else(|| panic!("no edge between nodes {a} and {b}"))
    }

    pub fn neighbors(&self, node: usize) -> impl Iterator<Item = usize> + '_ {
        self.edges.iter().filter_map(move |e| {
            if e.a == node {
                Some(e.b)
            } else if e.b == node {
                Some(e.a)
            } else {
                None
            }
        })
    }

    pub fn switch_at(&self, node: usize) -> Option<&Switch> {
        self.switches.iter().find(|s| s.node == node)
    }

    /// Where a train standing at `at` (having come from `came_from`) continues.
    /// `None` means terminus. Pass `came_from == at` for freshly spawned trains.
    pub fn next_node(&self, came_from: usize, at: usize) -> Option<usize> {
        if let Some(switch) = self.switch_at(at) {
            let target = switch.options[switch.selected];
            // A switch never routes a train back where it came from; if the
            // selected branch *is* the incoming one (trailing move), take the
            // other branch instead of reversing.
            if target != came_from {
                return Some(target);
            }
            return Some(switch.options[1 - switch.selected]);
        }
        // Plain nodes can have several continuations (e.g. a merge has the
        // other approach line as a neighbor too). Trains never reverse, so
        // take the straightest one: the largest dot product with the incoming
        // travel direction. Fresh spawns (`came_from == at`) have no incoming
        // direction; their sources only have a single neighbor anyway.
        let incoming = (self.pos(at) - self.pos(came_from)).normalize_or_zero();
        self.neighbors(at)
            .filter(|&n| n != came_from)
            .max_by(|&n1, &n2| {
                let d1 = incoming.dot((self.pos(n1) - self.pos(at)).normalize_or_zero());
                let d2 = incoming.dot((self.pos(n2) - self.pos(at)).normalize_or_zero());
                d1.total_cmp(&d2)
            })
    }
}

/// Hardcoded prototype level: two sources merge into one shared track, which
/// then splits to two color-coded terminal stations.
///
/// ```text
/// A (0)────(2)──╗signal                 ╔──(6)────(8) ORANGE
///               (4)═════════(5)switch═══╝
/// B (1)────(3)──╝signal                 ╚──(7)────(9) BLAU
/// ```
pub fn build_level() -> TrackGraph {
    let positions_kinds: [(f32, f32, NodeKind); 10] = [
        (-590.0, 130.0, NodeKind::Source("A")),
        (-590.0, -130.0, NodeKind::Source("B")),
        (-260.0, 130.0, NodeKind::Plain),
        (-260.0, -130.0, NodeKind::Plain),
        (-110.0, 0.0, NodeKind::Plain), // merge
        (140.0, 0.0, NodeKind::Plain),  // switch
        (340.0, 130.0, NodeKind::Plain),
        (340.0, -130.0, NodeKind::Plain),
        (560.0, 130.0, NodeKind::Sink(Cargo::Orange)),
        (560.0, -130.0, NodeKind::Sink(Cargo::Blue)),
    ];

    let nodes: Vec<Node> = positions_kinds
        .into_iter()
        .map(|(x, y, kind)| Node {
            pos: Vec2::new(x, y),
            kind,
        })
        .collect();

    let pairs = [
        (0, 2),
        (1, 3),
        (2, 4),
        (3, 4),
        (4, 5),
        (5, 6),
        (5, 7),
        (6, 8),
        (7, 9),
    ];
    let edges = pairs
        .into_iter()
        .map(|(a, b)| Edge {
            a,
            b,
            len: nodes[a].pos.distance(nodes[b].pos),
        })
        .collect();

    TrackGraph {
        nodes,
        edges,
        switches: vec![Switch {
            node: 5,
            options: [6, 7],
            selected: 0,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Walks a train from `source` to its terminus and returns the node path.
    fn walk(graph: &TrackGraph, source: usize) -> Vec<usize> {
        let mut path = vec![source];
        let (mut came_from, mut at) = (source, source);
        while let Some(next) = graph.next_node(came_from, at) {
            path.push(next);
            (came_from, at) = (at, next);
            assert!(path.len() < 100, "routing loop");
        }
        path
    }

    #[test]
    fn route_follows_switch_position() {
        let mut graph = build_level();

        // Switch toward the top branch: both sources end at the orange sink.
        assert_eq!(walk(&graph, 0), vec![0, 2, 4, 5, 6, 8]);
        assert_eq!(walk(&graph, 1), vec![1, 3, 4, 5, 6, 8]);

        graph.switches[0].selected = 1;
        assert_eq!(walk(&graph, 0), vec![0, 2, 4, 5, 7, 9]);
    }

    #[test]
    fn termini_are_the_color_sinks() {
        let graph = build_level();
        assert!(matches!(graph.nodes[8].kind, NodeKind::Sink(Cargo::Orange)));
        assert!(matches!(graph.nodes[9].kind, NodeKind::Sink(Cargo::Blue)));
        assert_eq!(graph.next_node(6, 8), None);
        assert_eq!(graph.next_node(7, 9), None);
    }

    #[test]
    fn edge_lengths_match_node_distances() {
        let graph = build_level();
        for edge in &graph.edges {
            let expected = graph.pos(edge.a).distance(graph.pos(edge.b));
            assert!((edge.len - expected).abs() < f32::EPSILON * 100.0);
        }
        assert_eq!(graph.edge_len(0, 2), graph.edge_len(2, 0));
    }
}
