//! The deterministic tick loop (plan §4.1).
//!
//! Phase order per tick — part of the API contract; changing it changes
//! gameplay and every replay hash:
//!
//! 1. **Spawn:** due schedule entries join their source's FIFO; the queue
//!    head enters when the entry edge is physically clear (GDD §7.5).
//! 2. **Signal claims:** trains standing at a red signal request clearance
//!    in (waiting-since, id) order — first come, first served (GDD §7.4).
//!    Granted chain signals reserve their route blocks here.
//! 3. **Movement:** trains advance in ascending id order; crossings through
//!    signals re-check clearance against live occupancy + claims.
//! 4. **Arrival** happens inline during movement (head reaches a sink
//!    anchor); wrong sink or dead end ⇒ misrouting.
//! 5. **Checks:** collision (interval overlap), deadlock (wait-for cycle),
//!    success, stall fallback. Then the replay hash is advanced.
//!
//! Block entry rule is *strict*: a block counts as busy even if only the
//! train itself occupies it. Normally a train's own body is never ahead of
//! it, so this changes nothing — but on a ring, where one cut does not
//! split the block, it stops a train from driving through its own tail.
//! That self-jam ends as `Stalled` (scenario 18), the diagnosable outcome.

use crate::failure;
use crate::graph::{self, Next, TrackGraph};
use crate::grid::Cell;
use crate::hash::Fnv1a64;
use crate::layout::{Layout, SignalKind, ValidationError};
use crate::level::{Level, ScheduleEntry};
use crate::routing::{RouteEnd, resolve, walk_route};
use crate::score::{Score, material_cost};
use crate::train::Train;
use crate::units::{BlockId, EdgeId, Len, STALL_TICKS, SinkId, SourceId, Tick, TrainId};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// All scheduled trains arrived at their correct sinks.
    Success { score: Score },
    /// Two train bodies overlap (edge reported in canonical direction).
    Collision {
        trains: (TrainId, TrainId),
        edge: EdgeId,
    },
    /// A train reached a wrong sink or a dead end (GDD §7.6).
    Misrouting {
        train: TrainId,
        /// What was actually reached (`None` = dead end).
        reached: Option<SinkId>,
        /// Last switch whose *other* branch would have led to the target.
        blame: Option<Cell>,
    },
    /// Trains wait on each other in a cycle; rotated to start at the
    /// smallest involved id.
    Deadlock { cycle: Vec<TrainId> },
    /// No movement for [`STALL_TICKS`] (or the run cap was reached) with an
    /// unfinished schedule, but no cycle — e.g. a self-jam on a ring block.
    Stalled { waiting: Vec<TrainId> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimEvent {
    TrainSpawned(TrainId),
    TrainArrived { train: TrainId, at: Tick },
    SignalBlocked { train: TrainId },
    RunEnded(Outcome),
}

pub struct Sim {
    graph: TrackGraph,
    /// Schedule sorted by (depart, train) — the order is contract.
    schedule: Vec<ScheduleEntry>,
    next_departure: usize,
    /// FIFO of schedule indices per source (GDD §7.5).
    queues: BTreeMap<SourceId, VecDeque<u32>>,
    trains: Vec<Train>, // always sorted by ascending id
    now: Tick,
    arrivals: Vec<(TrainId, Tick)>,
    lateness_total: u64,
    /// Chain-signal route reservations (GDD §7.4). A reservation is dropped
    /// once its owner occupies the block (occupancy takes over) or despawns.
    reservations: BTreeMap<BlockId, TrainId>,
    sink_by_arrival: BTreeMap<EdgeId, SinkId>,
    entry_by_source: BTreeMap<SourceId, EdgeId>,
    material: u32,
    stall_ticks: u64,
    outcome: Option<Outcome>,
    events: Vec<SimEvent>,
    hash: u64,
}

/// Live view used by clearance checks during one tick.
struct TickState {
    /// Block → trains touching it; extended live as trains move/spawn,
    /// never shrunk within the tick (conservative and deterministic).
    occupancy: BTreeMap<BlockId, BTreeSet<TrainId>>,
    /// Block-signal grants of this tick (phase 2 priority order).
    claims: BTreeMap<BlockId, TrainId>,
    /// Wait-for edges discovered this tick (blocked train → holder).
    waits: BTreeMap<TrainId, TrainId>,
    progressed: bool,
}

impl Sim {
    pub fn new(level: &Level, player: &Layout) -> Result<Sim, Vec<ValidationError>> {
        let graph = graph::build(level, player)?;
        let mut schedule = level.schedule.clone();
        schedule.sort_by_key(|e| (e.depart, e.train));
        let sink_by_arrival = graph.sinks.iter().map(|s| (s.arrival, s.id)).collect();
        let entry_by_source = graph.sources.iter().map(|s| (s.id, s.entry)).collect();
        Ok(Sim {
            graph,
            schedule,
            next_departure: 0,
            queues: BTreeMap::new(),
            trains: Vec::new(),
            now: Tick(0),
            arrivals: Vec::new(),
            lateness_total: 0,
            reservations: BTreeMap::new(),
            sink_by_arrival,
            entry_by_source,
            material: material_cost(player),
            stall_ticks: 0,
            outcome: None,
            events: Vec::new(),
            hash: Fnv1a64::new().finish(),
        })
    }

    pub fn now(&self) -> Tick {
        self.now
    }

    pub fn outcome(&self) -> Option<&Outcome> {
        self.outcome.as_ref()
    }

    pub fn trains(&self) -> &[Train] {
        &self.trains
    }

    pub fn graph(&self) -> &TrackGraph {
        &self.graph
    }

    pub fn replay_hash(&self) -> u64 {
        self.hash
    }

    /// Headless run until an outcome or `max` ticks. Hitting the cap with an
    /// unfinished schedule ends as `Stalled` (e.g. an endless runaway) — a
    /// run always terminates with a diagnosable outcome.
    pub fn run(&mut self, max: Tick) -> Outcome {
        while self.outcome.is_none() && self.now < max {
            self.step();
        }
        if self.outcome.is_none() {
            let waiting = self.trains.iter().map(|t| t.id).collect();
            self.finish(Outcome::Stalled { waiting });
        }
        self.outcome.clone().expect("set above")
    }

    pub fn step(&mut self) -> &[SimEvent] {
        self.events.clear();
        if self.outcome.is_some() {
            return &self.events;
        }
        self.now = Tick(self.now.0 + 1);

        let mut tick = TickState {
            occupancy: BTreeMap::new(),
            claims: BTreeMap::new(),
            waits: BTreeMap::new(),
            progressed: false,
        };
        for train in &self.trains {
            for (edge, lo, hi) in train.occupied(&self.graph) {
                if hi > lo {
                    tick.occupancy
                        .entry(self.graph.blocks.block_of(edge))
                        .or_default()
                        .insert(train.id);
                }
            }
            // The head commits its edge even at zero covered length — a
            // train that crossed exactly onto an edge start (head_dist 0)
            // must not look absent from that block one tick later.
            tick.occupancy
                .entry(self.graph.blocks.block_of(train.head_edge()))
                .or_default()
                .insert(train.id);
        }

        self.phase_spawn(&mut tick);
        self.phase_signal_claims(&mut tick);
        self.phase_movement(&mut tick);
        if self.outcome.is_none() {
            self.phase_checks(&mut tick);
        }
        self.advance_hash();
        self.events.as_slice()
    }

    // --- Phase 1 -----------------------------------------------------------

    fn phase_spawn(&mut self, tick: &mut TickState) {
        while self.next_departure < self.schedule.len()
            && self.schedule[self.next_departure].depart <= self.now
        {
            let entry = &self.schedule[self.next_departure];
            self.queues
                .entry(entry.source)
                .or_default()
                .push_back(self.next_departure as u32);
            self.next_departure += 1;
        }

        let sources: Vec<SourceId> = self.queues.keys().copied().collect();
        for source in sources {
            let Some(&index) = self.queues[&source].front() else {
                continue;
            };
            let entry_edge = self.entry_by_source[&source];
            if !self.edge_clear(entry_edge) {
                continue;
            }
            // At most one spawn per source per tick: the new train has zero
            // body length yet, so a second physical check would wrongly pass.
            self.queues.get_mut(&source).expect("exists").pop_front();
            let e = &self.schedule[index as usize];
            let train = Train {
                id: e.train,
                class: e.class,
                length: e.length,
                speed: e.speed,
                sink: e.sink,
                due: e.due,
                path: VecDeque::from([entry_edge]),
                head_dist: Len(0),
                passed_switches: Vec::new(),
                waiting_since: None,
            };
            tick.occupancy
                .entry(self.graph.blocks.block_of(entry_edge))
                .or_default()
                .insert(train.id);
            let position = self
                .trains
                .binary_search_by_key(&train.id, |t| t.id)
                .expect_err("train ids are unique (validated)");
            self.trains.insert(position, train);
            self.events.push(SimEvent::TrainSpawned(e.train));
            tick.progressed = true;
        }
        self.queues.retain(|_, q| !q.is_empty());
    }

    /// Physically clear: no train interval on the edge in either direction.
    fn edge_clear(&self, edge: EdgeId) -> bool {
        let opposite = self.graph.edge(edge).opposite;
        for train in &self.trains {
            for (e, lo, hi) in train.occupied(&self.graph) {
                if (e == edge || e == opposite) && hi > lo {
                    return false;
                }
            }
        }
        true
    }

    // --- Phase 2 -----------------------------------------------------------

    /// Trains already standing at a signal request clearance in
    /// (waiting_since, id) order — first come wins, ties go to the lower id.
    fn phase_signal_claims(&mut self, tick: &mut TickState) {
        let mut order: Vec<(u64, TrainId)> = self
            .trains
            .iter()
            .filter(|t| self.at_signal_end(t))
            .map(|t| (t.waiting_since.map_or(self.now.0, |w| w.0), t.id))
            .collect();
        order.sort();
        for (_, id) in order {
            let train = self.train(id).clone();
            let head = train.head_edge();
            // Arrival edges are handled in movement; a signal there is moot.
            if self.sink_by_arrival.contains_key(&head) {
                continue;
            }
            let Some(next_edge) = self.continuation(&train, head) else {
                continue;
            };
            if let Some(grant) = self.clearance(&train, head, next_edge, tick) {
                match grant {
                    Grant::Block(block) => {
                        tick.claims.insert(block, id);
                    }
                    Grant::Chain(blocks) => {
                        for block in blocks {
                            self.reservations.insert(block, id);
                        }
                    }
                }
            }
        }
    }

    fn at_signal_end(&self, train: &Train) -> bool {
        let edge = self.graph.edge(train.head_edge());
        edge.signal.is_some() && train.head_dist == edge.len
    }

    /// The edge a train would continue on after `edge` (switches resolved
    /// for this train). `None` = dead end.
    fn continuation(&self, train: &Train, edge: EdgeId) -> Option<EdgeId> {
        match self.graph.edge(edge).next {
            Next::Fixed(e) => Some(e),
            Next::SwitchChoice { switch } => Some(resolve(
                &self.graph.switches[switch as usize],
                train.class,
                train.sink,
            )),
            Next::DeadEnd => None,
        }
    }

    // --- Clearance ----------------------------------------------------------

    fn block_free(&self, block: BlockId, train: TrainId, tick: &TickState) -> bool {
        // Strict occupancy (see module docs), foreign claims and foreign
        // reservations all make a block busy; an own reservation is green.
        tick.occupancy.get(&block).is_none_or(|s| s.is_empty())
            && tick.claims.get(&block).is_none_or(|o| *o == train)
            && self.reservations.get(&block).is_none_or(|o| *o == train)
    }

    /// Holder of a busy block (for the wait-for graph): smallest *other*
    /// occupant id; claim/reservation owner as fallback. `None` for a pure
    /// self-jam — that is no wait-for edge (→ `Stalled`, not `Deadlock`).
    fn holder_of(&self, block: BlockId, train: TrainId, tick: &TickState) -> Option<TrainId> {
        if let Some(owners) = tick.occupancy.get(&block)
            && !owners.is_empty()
        {
            return owners.iter().copied().find(|&o| o != train);
        }
        match (tick.claims.get(&block), self.reservations.get(&block)) {
            (Some(&o), _) if o != train => Some(o),
            (_, Some(&o)) if o != train => Some(o),
            _ => None,
        }
    }

    /// Evaluates the signal on `signal_edge` for a crossing onto `next_edge`.
    /// `Some(grant)` = green (chain grants list the blocks to reserve).
    fn clearance(
        &self,
        train: &Train,
        signal_edge: EdgeId,
        next_edge: EdgeId,
        tick: &TickState,
    ) -> Option<Grant> {
        let signal_id = self.graph.edge(signal_edge).signal.expect("caller checked");
        match self.graph.signals[signal_id.0 as usize].kind {
            SignalKind::Block => {
                let block = self.graph.blocks.block_of(next_edge);
                if self.block_free(block, train.id, tick) {
                    Some(Grant::Block(block))
                } else {
                    None
                }
            }
            SignalKind::Chain => {
                let blocks = self.chain_route_blocks(train, next_edge);
                if blocks.iter().all(|&b| self.block_free(b, train.id, tick)) {
                    Some(Grant::Chain(blocks))
                } else {
                    None
                }
            }
        }
    }

    /// Blocks a chain signal must secure (GDD §7.4): along the train's
    /// effective route, through further chain signals, up to and including
    /// the block behind the first block signal — or up to the sink.
    fn chain_route_blocks(&self, train: &Train, first: EdgeId) -> Vec<BlockId> {
        let mut blocks = Vec::new();
        let push = |b: BlockId, blocks: &mut Vec<BlockId>| {
            if !blocks.contains(&b) {
                blocks.push(b);
            }
        };
        let mut current = first;
        for _ in 0..=self.graph.edges.len() {
            push(self.graph.blocks.block_of(current), &mut blocks);
            if self.sink_by_arrival.contains_key(&current) {
                break; // route ends in the world's edge — nothing beyond
            }
            if let Some(signal) = self.graph.edge(current).signal
                && matches!(
                    self.graph.signals[signal.0 as usize].kind,
                    SignalKind::Block
                )
            {
                // Include the block the block signal protects, then stop.
                if let Some(after) = self.continuation(train, current) {
                    push(self.graph.blocks.block_of(after), &mut blocks);
                }
                break;
            }
            match self.continuation(train, current) {
                Some(next) => current = next,
                None => break,
            }
        }
        blocks
    }

    // --- Phase 3 -----------------------------------------------------------

    fn phase_movement(&mut self, tick: &mut TickState) {
        let ids: Vec<TrainId> = self.trains.iter().map(|t| t.id).collect();
        for id in ids {
            if self.outcome.is_some() {
                return;
            }
            self.move_train(id, tick);
        }
    }

    fn move_train(&mut self, id: TrainId, tick: &mut TickState) {
        let mut budget = self.train(id).speed.0;
        loop {
            let head = self.train(id).head_edge();
            let head_len = self.graph.edge(head).len;
            let to_end = head_len.0 - self.train(id).head_dist.0;

            if to_end > 0 {
                if budget == 0 {
                    break;
                }
                let step = budget.min(to_end);
                self.train_mut(id).head_dist.0 += step;
                budget -= step;
                tick.progressed = true;
                continue;
            }

            // Head exactly at the edge end. End-of-edge handling runs even
            // with an exhausted budget: crossings cost no distance, and the
            // arrival tick is "head reaches the anchor" (plan §3.5), not
            // "head rests on the anchor one tick later".
            // Arrival beats everything else.
            if let Some(&sink) = self.sink_by_arrival.get(&head) {
                self.arrive(id, sink);
                return;
            }

            let Some(next_edge) = self.continuation(&self.train(id).clone(), head) else {
                let train = self.train(id).clone();
                let blame = self.blame(&train);
                self.finish(Outcome::Misrouting {
                    train: id,
                    reached: None,
                    blame,
                });
                return;
            };

            if self.graph.edge(head).signal.is_some() {
                let train = self.train(id).clone();
                match self.clearance(&train, head, next_edge, tick) {
                    Some(Grant::Block(block)) => {
                        tick.claims.insert(block, id);
                    }
                    Some(Grant::Chain(blocks)) => {
                        for block in blocks {
                            self.reservations.insert(block, id);
                        }
                    }
                    None => {
                        // Blocked: remember since when (first-come priority)
                        // and on whom we wait (deadlock detection).
                        if self.train(id).waiting_since.is_none() {
                            self.train_mut(id).waiting_since = Some(self.now);
                            self.events.push(SimEvent::SignalBlocked { train: id });
                        }
                        let needed = self.needed_blocks(&train, head, next_edge);
                        for block in needed {
                            if let Some(holder) = self.holder_of(block, id, tick) {
                                tick.waits.insert(id, holder);
                                break;
                            }
                        }
                        return;
                    }
                }
            }

            // Cross.
            if let Next::SwitchChoice { switch } = self.graph.edge(head).next {
                let cell = self.graph.switches[switch as usize].cell;
                self.train_mut(id).passed_switches.push((cell, next_edge));
            }
            let block = self.graph.blocks.block_of(next_edge);
            tick.occupancy.entry(block).or_default().insert(id);
            // Occupancy takes over from a reservation the moment we enter.
            if self.reservations.get(&block) == Some(&id) {
                self.reservations.remove(&block);
            }
            let train = self.train_mut(id);
            train.waiting_since = None;
            train.path.push_back(next_edge);
            train.head_dist = Len(0);
        }
        let graph = &self.graph;
        let train = self
            .trains
            .iter_mut()
            .find(|t| t.id == id)
            .expect("alive in this phase");
        train.trim_path(graph);
    }

    /// The blocks whose state caused a red signal (for wait-for edges).
    fn needed_blocks(&self, train: &Train, signal_edge: EdgeId, next_edge: EdgeId) -> Vec<BlockId> {
        let signal_id = self.graph.edge(signal_edge).signal.expect("caller checked");
        match self.graph.signals[signal_id.0 as usize].kind {
            SignalKind::Block => vec![self.graph.blocks.block_of(next_edge)],
            SignalKind::Chain => self.chain_route_blocks(train, next_edge),
        }
    }

    fn arrive(&mut self, id: TrainId, sink: SinkId) {
        let train = self.train(id).clone();
        if sink != train.sink {
            let blame = self.blame(&train);
            self.finish(Outcome::Misrouting {
                train: id,
                reached: Some(sink),
                blame,
            });
            return;
        }
        self.arrivals.push((id, self.now));
        self.lateness_total += self.now.0.saturating_sub(train.due.0);
        self.reservations.retain(|_, owner| *owner != id);
        self.trains.retain(|t| t.id != id);
        self.events.push(SimEvent::TrainArrived {
            train: id,
            at: self.now,
        });
    }

    /// Last passed switch whose *other* branch would have reached the
    /// target sink (walking backwards — plan §4.4).
    fn blame(&self, train: &Train) -> Option<Cell> {
        for &(cell, taken) in train.passed_switches.iter().rev() {
            let switch = self
                .graph
                .switches
                .iter()
                .find(|s| s.cell == cell)
                .expect("recorded from this graph");
            let other = if switch.branch_out[0] == taken {
                switch.branch_out[1]
            } else {
                switch.branch_out[0]
            };
            if walk_route(&self.graph, other, train.class, train.sink) == RouteEnd::Sink(train.sink)
            {
                return Some(cell);
            }
        }
        None
    }

    // --- Phase 5 -----------------------------------------------------------

    fn phase_checks(&mut self, tick: &mut TickState) {
        // Collision: strict interval overlap on canonical edges.
        let mut per_edge: BTreeMap<EdgeId, Vec<(TrainId, i64, i64)>> = BTreeMap::new();
        for train in &self.trains {
            for (edge, lo, hi) in train.occupied(&self.graph) {
                let data = self.graph.edge(edge);
                let (canonical, lo, hi) = if edge <= data.opposite {
                    (edge, lo.0, hi.0)
                } else {
                    (data.opposite, data.len.0 - hi.0, data.len.0 - lo.0)
                };
                per_edge
                    .entry(canonical)
                    .or_default()
                    .push((train.id, lo, hi));
            }
        }
        for (edge, intervals) in &per_edge {
            for (i, &(id_a, lo_a, hi_a)) in intervals.iter().enumerate() {
                for &(id_b, lo_b, hi_b) in &intervals[i + 1..] {
                    if id_a != id_b && lo_a < hi_b && lo_b < hi_a {
                        let pair = (id_a.min(id_b), id_a.max(id_b));
                        self.finish(Outcome::Collision {
                            trains: pair,
                            edge: *edge,
                        });
                        return;
                    }
                }
            }
        }

        // Deadlock: cycle in this tick's wait-for graph.
        if let Some(cycle) = failure::find_cycle(&tick.waits) {
            self.finish(Outcome::Deadlock { cycle });
            return;
        }

        // Success: schedule done, world empty.
        if self.next_departure == self.schedule.len()
            && self.queues.is_empty()
            && self.trains.is_empty()
        {
            let throughput = self
                .arrivals
                .iter()
                .map(|&(_, tick)| tick)
                .max()
                .unwrap_or(self.now);
            self.finish(Outcome::Success {
                score: Score {
                    throughput,
                    material: self.material,
                    lateness: self.lateness_total,
                },
            });
            return;
        }

        // Stall fallback.
        if tick.progressed {
            self.stall_ticks = 0;
        } else {
            self.stall_ticks += 1;
            if self.stall_ticks >= STALL_TICKS {
                let waiting = self.trains.iter().map(|t| t.id).collect();
                self.finish(Outcome::Stalled { waiting });
            }
        }
    }

    fn finish(&mut self, outcome: Outcome) {
        self.events.push(SimEvent::RunEnded(outcome.clone()));
        self.outcome = Some(outcome);
    }

    // --- Hash ---------------------------------------------------------------

    /// Canonical bytes of the complete mutable state, in documented order.
    /// What is missing here is by definition *not* state — this function
    /// answers "what would a savegame have to store?".
    fn canonical_bytes(&self, h: &mut Fnv1a64) {
        h.write_u64(self.now.0);
        h.write_u32(self.trains.len() as u32);
        for t in &self.trains {
            h.write_u32(t.id.0);
            h.write_u32(t.class.0);
            h.write_i64(t.length.0);
            h.write_i64(t.speed.0);
            h.write_u32(t.sink.0);
            h.write_u64(t.due.0);
            h.write_i64(t.head_dist.0);
            h.write_u64(t.waiting_since.map_or(u64::MAX, |w| w.0));
            h.write_u32(t.path.len() as u32);
            for e in &t.path {
                h.write_u32(e.0);
            }
            h.write_u32(t.passed_switches.len() as u32);
            for (cell, edge) in &t.passed_switches {
                h.write(&cell.x.to_le_bytes());
                h.write(&cell.y.to_le_bytes());
                h.write_u32(edge.0);
            }
        }
        h.write_u32(self.reservations.len() as u32);
        for (&block, &owner) in &self.reservations {
            h.write_u32(block.0);
            h.write_u32(owner.0);
        }
        h.write_u32(self.queues.len() as u32);
        for (&source, queue) in &self.queues {
            h.write_u32(source.0);
            h.write_u32(queue.len() as u32);
            for &index in queue {
                h.write_u32(index);
            }
        }
        h.write_u32(self.next_departure as u32);
        h.write_u32(self.arrivals.len() as u32);
        for &(train, tick) in &self.arrivals {
            h.write_u32(train.0);
            h.write_u64(tick.0);
        }
        h.write_u64(self.lateness_total);
        h.write_u64(self.stall_ticks);
    }

    /// hash ← FNV(hash ‖ state): advanced once per tick, so the final value
    /// commits to the entire history, not just the final state.
    fn advance_hash(&mut self) {
        let mut h = Fnv1a64::new();
        h.write_u64(self.hash);
        self.canonical_bytes(&mut h);
        self.hash = h.finish();
    }

    // --- Helpers --------------------------------------------------------------

    fn train(&self, id: TrainId) -> &Train {
        self.trains
            .iter()
            .find(|t| t.id == id)
            .expect("caller guarantees the train is alive")
    }

    fn train_mut(&mut self, id: TrainId) -> &mut Train {
        self.trains
            .iter_mut()
            .find(|t| t.id == id)
            .expect("caller guarantees the train is alive")
    }
}

enum Grant {
    Block(BlockId),
    Chain(Vec<BlockId>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Dir8;
    use crate::layout::{SignalDef, SwitchDef, TrackPiece};
    use crate::level::{Par, ScheduleEntry, SinkDef, SourceDef};
    use crate::units::{Speed, TrainClass};

    fn cell(x: i32, y: i32) -> Cell {
        Cell { x, y }
    }

    /// Symmetric Y merge: two sources at equal distance from a merge switch,
    /// both gated by a block signal guarding the same shared block.
    fn merge_level() -> (Level, Layout) {
        let layout = Layout {
            pieces: vec![
                TrackPiece {
                    cell: cell(0, 1),
                    a: Dir8::W,
                    b: Dir8::SE,
                },
                TrackPiece {
                    cell: cell(0, -1),
                    a: Dir8::W,
                    b: Dir8::NE,
                },
                TrackPiece {
                    cell: cell(2, 0),
                    a: Dir8::W,
                    b: Dir8::E,
                },
            ],
            switches: vec![SwitchDef {
                cell: cell(1, 0),
                stem: Dir8::E,
                branches: [Dir8::NW, Dir8::SW],
                default_branch: 0,
                rules: vec![],
            }],
            signals: vec![
                SignalDef {
                    cell: cell(0, 1),
                    at: Dir8::SE,
                    kind: SignalKind::Block,
                },
                SignalDef {
                    cell: cell(0, -1),
                    at: Dir8::NE,
                    kind: SignalKind::Block,
                },
            ],
        };
        let level = Level {
            name: "merge".into(),
            buildable: vec![cell(0, 1), cell(0, -1), cell(1, 0), cell(2, 0)],
            fixed: Layout::default(),
            sources: vec![
                SourceDef {
                    id: SourceId(0),
                    cell: cell(0, 1),
                    dir: Dir8::W,
                },
                SourceDef {
                    id: SourceId(1),
                    cell: cell(0, -1),
                    dir: Dir8::W,
                },
            ],
            sinks: vec![SinkDef {
                id: SinkId(0),
                cell: cell(2, 0),
                dir: Dir8::E,
                label: "OST".into(),
            }],
            schedule: vec![
                ScheduleEntry {
                    train: TrainId(0),
                    class: TrainClass(0),
                    length: Len(400),
                    speed: Speed(100),
                    source: SourceId(0),
                    sink: SinkId(0),
                    depart: Tick(0),
                    due: Tick(200),
                },
                ScheduleEntry {
                    train: TrainId(1),
                    class: TrainClass(0),
                    length: Len(400),
                    speed: Speed(100),
                    source: SourceId(1),
                    sink: SinkId(0),
                    depart: Tick(0),
                    due: Tick(200),
                },
            ],
            par: Par {
                throughput: Tick(200),
                material: 0,
                lateness: 0,
            },
        };
        (level, layout)
    }

    /// Both trains reach their signals in the same tick and want the same
    /// block: the lower id must win, the other must wait (GDD §7.4).
    #[test]
    fn same_tick_contention_goes_to_lower_id() {
        let (level, layout) = merge_level();
        let mut sim = Sim::new(&level, &layout).expect("valid");
        // Both signals sit 1207 LE from the sources; at speed 100 both heads
        // stand at the signal after tick 13 (blocked mid-tick).
        for _ in 0..13 {
            sim.step();
        }
        let t0 = &sim.trains()[0];
        let t1 = &sim.trains()[1];
        // t0 crossed the signal (93 LE onto the switch stub; the fully left
        // approach edges were trimmed). t1 stands exactly at its signal.
        assert_eq!(t0.path.len(), 2, "t0 crossed the signal");
        assert_eq!(t0.head_dist, Len(93));
        assert_eq!(t1.path.len(), 1, "t1 held at its signal");
        assert_eq!(t1.head_dist, Len(707));
        assert!(t1.waiting_since.is_some(), "t1 is registered as waiting");

        // Both eventually arrive — and the run is a success.
        let outcome = sim.run(Tick(10_000));
        match outcome {
            Outcome::Success { .. } => {}
            other => panic!("expected success, got {other:?}"),
        }
    }
}
