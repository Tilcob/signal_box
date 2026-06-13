//! A train is an interval on a path of directed edges (plan §3.5): the head
//! sits at `head_dist` on `path.back()`, the body extends `length` backwards.

use crate::graph::TrackGraph;
use crate::grid::Cell;
use crate::units::{EdgeId, Len, SinkId, Speed, Tick, TrainClass, TrainId};
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct Train {
    pub id: TrainId,
    pub class: TrainClass,
    pub length: Len,
    pub speed: Speed,
    pub sink: SinkId,
    pub due: Tick,
    /// Edges the train touches: front = tail, back = head.
    pub path: VecDeque<EdgeId>,
    /// Head position on `path.back()`, measured from that edge's start.
    pub head_dist: Len,
    /// Switches passed via `SwitchChoice`, with the taken outgoing edge —
    /// the data misrouting blame is reconstructed from. Grows with every
    /// crossing and is deliberately NOT part of the per-tick replay hash
    /// (see `Sim::canonical_bytes`); it is derivable from the hashed path
    /// history.
    pub passed_switches: Vec<(Cell, EdgeId)>,
    /// Tick since when the train waits at its current red signal (first-come
    /// priority, GDD §7.4). Cleared on every crossing.
    pub waiting_since: Option<Tick>,
}

impl Train {
    pub fn head_edge(&self) -> EdgeId {
        *self.path.back().expect("a train always has a path")
    }

    /// Occupied intervals `(edge, from, to)` measured from each edge's
    /// start, head edge first. The tail ends at the start of `path.front()`
    /// at the latest — a freshly spawned train therefore grows into the
    /// world (plan §3.5).
    pub fn occupied(&self, graph: &TrackGraph) -> Vec<(EdgeId, Len, Len)> {
        let mut out = Vec::new();
        self.occupied_into(graph, &mut out);
        out
    }

    /// Like [`Train::occupied`] but fills `out` (cleared first) instead of
    /// allocating — hot paths (the per-tick occupancy build, collision check,
    /// the per-frame board render) reuse a single buffer across all trains.
    /// Output is bit-identical to `occupied`; do not let the two drift, or
    /// the replay hash changes.
    pub fn occupied_into(&self, graph: &TrackGraph, out: &mut Vec<(EdgeId, Len, Len)>) {
        out.clear();
        let mut rest = self.length.0;
        for (i, &edge) in self.path.iter().rev().enumerate() {
            let edge_len = graph.edge(edge).len.0;
            let (lo, hi) = if i == 0 {
                ((self.head_dist.0 - rest).max(0), self.head_dist.0)
            } else {
                ((edge_len - rest).max(0), edge_len)
            };
            if hi > lo {
                out.push((edge, Len(lo), Len(hi)));
            }
            rest -= hi - lo.max(0);
            if rest <= 0 {
                break;
            }
        }
    }

    /// Drops tail edges the train has fully left — otherwise the path (and
    /// from W4 on the replay hash) grows without bound.
    pub fn trim_path(&mut self, graph: &TrackGraph) {
        let mut rest = self.length.0;
        let mut keep = 1;
        for (i, &edge) in self.path.iter().rev().enumerate() {
            keep = i + 1;
            let edge_len = graph.edge(edge).len.0;
            let consumed = if i == 0 {
                self.head_dist.0.min(rest).max(0)
            } else {
                edge_len.min(rest)
            };
            rest -= consumed;
            if rest <= 0 {
                break;
            }
        }
        while self.path.len() > keep {
            self.path.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::build;
    use crate::grid::Dir8;
    use crate::layout::{Layout, TrackPiece};
    use crate::level::{Level, Par, ScheduleEntry, SinkDef, SourceDef};
    use crate::units::{SinkId, SourceId};

    fn line_graph(n: i32) -> TrackGraph {
        let cell = |x: i32| Cell { x, y: 0 };
        let layout = Layout {
            pieces: (0..n)
                .map(|x| TrackPiece {
                    cell: cell(x),
                    a: Dir8::W,
                    b: Dir8::E,
                })
                .collect(),
            switches: vec![],
            signals: vec![],
        };
        let level = Level {
            name: "t".into(),
            buildable: (0..n).map(cell).collect(),
            fixed: Layout::default(),
            sources: vec![SourceDef {
                id: SourceId(0),
                cell: cell(0),
                dir: Dir8::W,
            }],
            sinks: vec![SinkDef {
                id: SinkId(0),
                cell: cell(n - 1),
                dir: Dir8::E,
                label: "E".into(),
            }],
            schedule: vec![ScheduleEntry {
                train: TrainId(0),
                class: TrainClass(0),
                length: Len(800),
                speed: Speed(100),
                source: SourceId(0),
                sink: SinkId(0),
                depart: Tick(0),
                due: Tick(100),
            }],
            par: Par {
                throughput: Tick(100),
                material: n as u32,
                lateness: 0,
            },
        };
        build(&level, &layout).expect("valid")
    }

    #[test]
    fn occupied_grows_in_from_the_source() {
        let graph = line_graph(3);
        let entry = graph.sources[0].entry;
        let mut train = Train {
            id: TrainId(0),
            class: TrainClass(0),
            length: Len(800),
            speed: Speed(100),
            sink: SinkId(0),
            due: Tick(100),
            path: VecDeque::from([entry]),
            head_dist: Len(0),
            passed_switches: vec![],
            waiting_since: None,
        };
        assert!(train.occupied(&graph).is_empty(), "not yet in the world");

        train.head_dist = Len(300);
        assert_eq!(train.occupied(&graph), vec![(entry, Len(0), Len(300))]);
    }

    #[test]
    fn occupied_spans_edges_and_trim_drops_left_tail() {
        let graph = line_graph(3);
        let entry = graph.sources[0].entry; // 500 LE stub
        let e1 = match graph.edge(entry).next {
            crate::graph::Next::Fixed(e) => e,
            _ => unreachable!(),
        };
        let mut train = Train {
            id: TrainId(0),
            class: TrainClass(0),
            length: Len(800),
            speed: Speed(100),
            sink: SinkId(0),
            due: Tick(100),
            path: VecDeque::from([entry, e1]),
            head_dist: Len(400), // on e1 (500 long)
            passed_switches: vec![],
            waiting_since: None,
        };
        // 400 on e1 + 400 of the entry stub [100..500].
        assert_eq!(
            train.occupied(&graph),
            vec![(e1, Len(0), Len(400)), (entry, Len(100), Len(500))]
        );
        train.trim_path(&graph);
        assert_eq!(train.path.len(), 2, "entry still partially occupied");

        train.head_dist = Len(500);
        // Tail now exactly at the entry/e1 joint: entry fully left…
        train.path.push_back(match graph.edge(e1).next {
            crate::graph::Next::Fixed(e) => e,
            _ => unreachable!(),
        });
        train.head_dist = Len(300); // 300 + 500 = 800: tail at entry end
        train.trim_path(&graph);
        assert_eq!(train.path.len(), 2, "entry edge dropped");
    }
}
