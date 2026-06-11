//! Switch routing ("the switch is the program", GDD §7.3) and the route
//! walk used for the editor's reachability check and misrouting blame.

use crate::graph::{Next, SwitchData, TrackGraph};
use crate::layout::{Layout, RuleWhen, ValidationError};
use crate::level::Level;
use crate::units::{EdgeId, SinkId, TrainClass, TrainId};
use std::collections::BTreeMap;

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
/// configuration — the editor's pre-flight warning (GDD §7.3).
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
}
