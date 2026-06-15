//! Switch routing ("the switch is the program") and the route
//! walk used for the editor's reachability check and misrouting blame.

use crate::graph::{Next, SwitchData, TrackGraph};
use crate::grid::Cell;
use crate::layout::{Layout, RuleWhen, ValidationError};
use crate::level::Level;
use crate::units::{EdgeId, SinkId, TrainClass, TrainId};
use std::collections::{BTreeMap, BTreeSet};

/// Switch decision for a train: first matching rule wins (list order =
/// player priority), otherwise the default branch. Returns the outgoing
/// edge toward the chosen branch.
pub fn resolve(switch: &SwitchData, class: TrainClass, sink: SinkId) -> EdgeId {
    for rule in &switch.rules {
        let matches = match rule.when {
            RuleWhen::DestIs(s) => s == sink,
            RuleWhen::ClassIs(c) => c == class,
        };
        if matches {
            return switch.branch_out[rule.branch as usize];
        }
    }
    switch.branch_out[switch.default_branch as usize]
}

/// Where a route ends when followed from `start` with a given train profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteEnd {
    /// First sink anchor reached on the way.
    Sink(SinkId),
    DeadEnd,
    /// The loop guard tripped (more steps than edges exist).
    Loops,
}

/// Follows `Next` + [`resolve`] from a directed edge to its end — exactly
/// the path a train with these properties would drive. No search, no
/// backtracking: routing is deterministic, a route is a walk.
pub fn walk_route(graph: &TrackGraph, start: EdgeId, class: TrainClass, sink: SinkId) -> RouteEnd {
    let arrival: BTreeMap<EdgeId, SinkId> = graph.sinks.iter().map(|s| (s.arrival, s.id)).collect();

    let mut current = start;
    for _ in 0..=graph.edges.len() {
        if let Some(&reached) = arrival.get(&current) {
            return RouteEnd::Sink(reached);
        }
        current = match graph.edge(current).next {
            Next::Fixed(e) => e,
            Next::SwitchChoice { switch } => resolve(&graph.switches[switch as usize], class, sink),
            Next::DeadEnd => return RouteEnd::DeadEnd,
        };
    }
    RouteEnd::Loops
}

/// A scheduled train that cannot reach its sink with the current switch
/// configuration — the editor's pre-flight warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unreachable {
    pub train: TrainId,
    pub end: RouteEnd,
}

/// Editor pre-check: walks every scheduled train's route from its source.
/// `Err` = the layout does not even validate (the editor shows those errors
/// instead).
pub fn check_reachability(
    level: &Level,
    player: &Layout,
) -> Result<Vec<Unreachable>, Vec<ValidationError>> {
    let graph = crate::graph::build(level, player)?;
    let entry: BTreeMap<_, _> = graph.sources.iter().map(|s| (s.id, s.entry)).collect();

    let mut out = Vec::new();
    for e in &level.schedule {
        let start = entry[&e.source];
        match walk_route(&graph, start, e.class, e.sink) {
            RouteEnd::Sink(reached) if reached == e.sink => {}
            end => out.push(Unreachable {
                train: e.train,
                end,
            }),
        }
    }
    Ok(out)
}

/// Sinks reachable *downstream* of a switch — i.e. lying behind it, entered
/// through one of its branches. A forward flood from both branch exits;
/// every sink whose arrival edge is met counts. Switches met on the way are
/// treated as open (both branches explored), so the result is independent of
/// the current routing rules: it answers "which destinations could this
/// switch ever steer to", not "where does it steer now".
///
/// Sinks reachable only by leaving through the stem (i.e. lying *before* the
/// switch) are excluded — listing them as routing targets only confuses, as
/// no branch can send a train there (see levels 2.1 / 2.3).
pub fn reachable_sinks_from_switch(graph: &TrackGraph, switch_index: usize) -> BTreeSet<SinkId> {
    let arrival: BTreeMap<EdgeId, SinkId> = graph.sinks.iter().map(|s| (s.arrival, s.id)).collect();
    let mut reached = BTreeSet::new();
    let mut visited: BTreeSet<EdgeId> = BTreeSet::new();
    let mut stack: Vec<EdgeId> = graph.switches[switch_index].branch_out.to_vec();
    while let Some(edge) = stack.pop() {
        if !visited.insert(edge) {
            continue;
        }
        if let Some(&sink) = arrival.get(&edge) {
            reached.insert(sink);
        }
        match graph.edge(edge).next {
            Next::Fixed(e) => stack.push(e),
            Next::SwitchChoice { switch } => {
                stack.extend(graph.switches[switch as usize].branch_out);
            }
            Next::DeadEnd => {}
        }
    }
    reached
}

/// Editor helper: the downstream sinks of the switch at `switch_cell`. `Err`
/// = the layout does not validate yet (the caller falls back to all sinks);
/// `Ok(None)` = no switch sits on that cell.
pub fn reachable_sinks(
    level: &Level,
    player: &Layout,
    switch_cell: Cell,
) -> Result<Option<BTreeSet<SinkId>>, Vec<ValidationError>> {
    let graph = crate::graph::build(level, player)?;
    Ok(graph
        .switches
        .iter()
        .position(|s| s.cell == switch_cell)
        .map(|i| reachable_sinks_from_switch(&graph, i)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::SwitchData;
    use crate::grid::Cell;
    use crate::layout::SwitchRule;
    use crate::units::NodeId;

    fn switch(default_branch: u8, rules: Vec<SwitchRule>) -> SwitchData {
        SwitchData {
            cell: Cell { x: 0, y: 0 },
            center: NodeId(0),
            stem_out: EdgeId(100),
            branch_out: [EdgeId(0), EdgeId(1)],
            default_branch,
            rules,
        }
    }

    #[test]
    fn empty_rules_use_default() {
        assert_eq!(
            resolve(&switch(1, vec![]), TrainClass(0), SinkId(0)),
            EdgeId(1)
        );
    }

    #[test]
    fn dest_rule_matches() {
        let sw = switch(
            0,
            vec![SwitchRule {
                when: RuleWhen::DestIs(SinkId(7)),
                branch: 1,
            }],
        );
        assert_eq!(resolve(&sw, TrainClass(0), SinkId(7)), EdgeId(1));
        assert_eq!(resolve(&sw, TrainClass(0), SinkId(8)), EdgeId(0));
    }

    #[test]
    fn class_rule_matches() {
        let sw = switch(
            0,
            vec![SwitchRule {
                when: RuleWhen::ClassIs(TrainClass(2)),
                branch: 1,
            }],
        );
        assert_eq!(resolve(&sw, TrainClass(2), SinkId(0)), EdgeId(1));
        assert_eq!(resolve(&sw, TrainClass(3), SinkId(0)), EdgeId(0));
    }

    #[test]
    fn first_matching_rule_wins_in_both_orders() {
        let dest = SwitchRule {
            when: RuleWhen::DestIs(SinkId(7)),
            branch: 0,
        };
        let class = SwitchRule {
            when: RuleWhen::ClassIs(TrainClass(1)),
            branch: 1,
        };
        // A train matching BOTH rules gets whichever comes first.
        let sw = switch(0, vec![dest, class]);
        assert_eq!(resolve(&sw, TrainClass(1), SinkId(7)), EdgeId(0));
        let sw = switch(0, vec![class, dest]);
        assert_eq!(resolve(&sw, TrainClass(1), SinkId(7)), EdgeId(1));
    }

    #[test]
    fn reachable_sinks_excludes_targets_before_the_switch() {
        use crate::grid::Dir8;
        use crate::layout::{SwitchDef, TrackPiece};
        use crate::level::{Par, ScheduleEntry, SinkDef, SourceDef};
        use crate::units::{Len, SourceId, Speed, Tick};

        let cell = |x, y| Cell { x, y };
        // source (0,0)W → switch (1,0) stem W, branches E→(2,0), NE→(2,1).
        // sink 0 sits behind the straight branch, sink 1 behind the diagonal
        // branch, sink 2 on the stem cell itself — before the switch.
        let layout = Layout {
            pieces: vec![
                TrackPiece {
                    cell: cell(0, 0),
                    a: Dir8::W,
                    b: Dir8::E,
                },
                TrackPiece {
                    cell: cell(2, 0),
                    a: Dir8::W,
                    b: Dir8::E,
                },
                TrackPiece {
                    cell: cell(2, 1),
                    a: Dir8::SW,
                    b: Dir8::NE,
                },
            ],
            switches: vec![SwitchDef {
                cell: cell(1, 0),
                stem: Dir8::W,
                branches: [Dir8::E, Dir8::NE],
                default_branch: 0,
                rules: vec![],
            }],
            signals: vec![],
        };
        let level = Level {
            name: "t".into(),
            buildable: vec![cell(0, 0), cell(1, 0), cell(2, 0), cell(2, 1)],
            fixed: Layout::default(),
            sources: vec![SourceDef {
                id: SourceId(0),
                cell: cell(0, 0),
                dir: Dir8::W,
            }],
            sinks: vec![
                SinkDef {
                    id: SinkId(0),
                    cell: cell(2, 0),
                    dir: Dir8::E,
                    label: "OST".into(),
                },
                SinkDef {
                    id: SinkId(1),
                    cell: cell(2, 1),
                    dir: Dir8::NE,
                    label: "NORD".into(),
                },
                SinkDef {
                    id: SinkId(2),
                    cell: cell(0, 0),
                    dir: Dir8::W,
                    label: "WEST".into(),
                },
            ],
            schedule: vec![ScheduleEntry {
                train: TrainId(0),
                class: TrainClass(0),
                length: Len(400),
                speed: Speed(100),
                source: SourceId(0),
                sink: SinkId(0),
                depart: Tick(0),
                due: Tick(200),
            }],
            par: Par {
                throughput: Tick(0),
                material: 0,
                lateness: 0,
            },
        };

        let reachable = reachable_sinks(&level, &layout, cell(1, 0)).expect("valid");
        let reachable = reachable.expect("switch exists on that cell");
        assert!(reachable.contains(&SinkId(0)), "straight branch sink");
        assert!(reachable.contains(&SinkId(1)), "diagonal branch sink");
        assert!(
            !reachable.contains(&SinkId(2)),
            "stem-side sink is before the switch"
        );

        // No switch on an empty cell → None, not an error.
        assert_eq!(reachable_sinks(&level, &layout, cell(2, 0)), Ok(None));
    }
}
