//! The deterministic tick loop.
//!
//! Phase order per tick — part of the API contract; changing it changes
//! gameplay and every replay hash:
//!
//! 1. **Spawn:** due schedule entries join their source's FIFO; the queue
//!    head enters when the entry edge is physically clear and its block is
//!    not chain-reserved for another train.
//! 2. **Signal claims:** trains standing at a red signal request clearance
//!    in (waiting-since, id) order — first come, first served.
//!    Granted chain signals reserve their route blocks here.
//! 3. **Movement:** trains advance in ascending id order; crossings through
//!    signals re-check clearance against live occupancy + claims.
//! 4. **Arrival** happens inline during movement (head reaches a sink
//!    anchor); wrong sink or dead end ⇒ misrouting.
//! 5. **Checks:** collision (edge interval overlap + shared interior crossing
//!    node), deadlock (wait-for cycle), success, stall fallback. Then the
//!    replay hash is advanced.
//!
//! Block entry rule is *strict*: a block counts as busy even if only the
//! train itself occupies it. Normally a train's own body is never ahead of
//! it, so this changes nothing — but on a ring, where one cut does not
//! split the block, it stops a train from driving through its own tail.
//! That self-jam ends as `Stalled`, the diagnosable outcome.

use crate::failure;
use crate::graph::{self, Next, TrackGraph};
use crate::grid::Cell;
use crate::hash::Fnv1a64;
use crate::layout::{Layout, SignalKind, ValidationError};
use crate::level::{Level, ScheduleEntry};
use crate::routing::{RouteEnd, resolve, walk_route};
use crate::score::{Score, material_cost};
use crate::train::{PendingStop, Train};
use crate::units::{
    BlockId, EdgeId, Len, NodeId, PlatformId, STALL_TICKS, SinkId, SourceId, Tick, TrainId,
};
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
    /// A train reached a wrong sink or a dead end.
    Misrouting {
        train: TrainId,
        /// What was actually reached (`None` = dead end).
        reached: Option<SinkId>,
        /// Last switch whose *other* branch would have led to the target.
        blame: Option<Cell>,
    },
    /// Freight: a train reached its correct sink without completing its
    /// mandatory unload stop — its route never crossed the assigned platform.
    FreightNotDelivered {
        train: TrainId,
        platform: PlatformId,
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
    /// FIFO of schedule indices per source.
    queues: BTreeMap<SourceId, VecDeque<u32>>,
    trains: Vec<Train>, // always sorted by ascending id
    now: Tick,
    arrivals: Vec<(TrainId, Tick)>,
    lateness_total: u64,
    /// Chain-signal route reservations. A reservation is dropped
    /// once its owner occupies the block (occupancy takes over) or despawns.
    reservations: BTreeMap<BlockId, TrainId>,
    /// Crossing-point reservations from chain grants: `node → (owner, occupied)`.
    /// `occupied` flips true once the owner's body covers the point; the
    /// reservation is dropped the first tick after it stops covering it (the
    /// owner has passed), or on arrival. Block grants use the per-tick point
    /// claim instead, so they never appear here.
    point_reservations: BTreeMap<NodeId, (TrainId, bool)>,
    sink_by_arrival: BTreeMap<EdgeId, SinkId>,
    /// Freight platform → its arrival edge (center → connector). Used at spawn
    /// to seed a train's [`PendingStop::arrival_edge`] from its schedule entry.
    platform_arrival: BTreeMap<PlatformId, EdgeId>,
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
    /// Crossing point → trains whose body covers it (built from bodies, extended
    /// live as trains cross). The cross-block conflict resource at a flat crossing.
    point_occupancy: BTreeMap<NodeId, BTreeSet<TrainId>>,
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
        let platform_arrival = graph.platforms.iter().map(|p| (p.id, p.arrival)).collect();
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
            point_reservations: BTreeMap::new(),
            sink_by_arrival,
            platform_arrival,
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

    /// Active chain-signal reservations (read-only, for the frontend's
    /// block lighting — reservations stay visible permanently).
    pub fn reservations(&self) -> &BTreeMap<BlockId, TrainId> {
        &self.reservations
    }

    /// Per-train arrival ticks, in arrival order. Empty until trains finish;
    /// after a `Success` run it holds one entry per scheduled train. Used by
    /// the `due_suggest` authoring tool to derive timetable `due` values from a
    /// reference solution.
    pub fn arrivals(&self) -> &[(TrainId, Tick)] {
        &self.arrivals
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
            point_occupancy: BTreeMap::new(),
            waits: BTreeMap::new(),
            progressed: false,
        };
        let mut occ_buf = Vec::new();
        for train in &self.trains {
            train.occupied_into(&self.graph, &mut occ_buf);
            for &(edge, lo, hi) in &occ_buf {
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
            // Crossing points the body strictly covers: the joints between
            // consecutive occupied edges (head tip / tail end excluded) that are
            // flat-crossing centres.
            for pair in occ_buf.windows(2) {
                let node = self.graph.edge(pair[0].0).from;
                if self.graph.crossing_nodes.contains(&node) {
                    tick.point_occupancy.entry(node).or_default().insert(train.id);
                }
            }
        }
        // Release chain point reservations whose owner has passed the point:
        // `occupied` flips true while covering it, then the reservation drops the
        // first tick it no longer covers it. Uses this tick's body snapshot, so
        // it over-holds by at most one tick — never under-holds.
        self.point_reservations.retain(|node, (owner, occupied)| {
            let covering = tick
                .point_occupancy
                .get(node)
                .is_some_and(|s| s.contains(owner));
            if covering {
                *occupied = true;
            }
            covering || !*occupied
        });

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

        if self.queues.is_empty() {
            return;
        }
        // Physically busy edges (either direction), computed once per tick —
        // a per-source scan over all trains would be O(sources × trains).
        // Same-tick spawns never appear here (zero body length), which keeps
        // the "one spawn per source per tick" rule below sound.
        let mut busy: BTreeSet<EdgeId> = BTreeSet::new();
        let mut occ_buf = Vec::new();
        for train in &self.trains {
            train.occupied_into(&self.graph, &mut occ_buf);
            for &(edge, lo, hi) in &occ_buf {
                if hi > lo {
                    busy.insert(edge);
                    busy.insert(self.graph.edge(edge).opposite);
                }
            }
        }

        let sources: Vec<SourceId> = self.queues.keys().copied().collect();
        for source in sources {
            let Some(&index) = self.queues[&source].front() else {
                continue;
            };
            let entry_edge = self.entry_by_source[&source];
            if busy.contains(&entry_edge) {
                continue;
            }
            // A chain reservation makes the entry block busy: its
            // owner crosses block boundaries without re-checking clearance,
            // so a train spawning here could not be protected by any signal.
            if self
                .reservations
                .contains_key(&self.graph.blocks.block_of(entry_edge))
            {
                continue;
            }
            // At most one spawn per source per tick: the new train has zero
            // body length yet, so a second physical check would wrongly pass.
            self.queues.get_mut(&source).expect("exists").pop_front();
            let e = &self.schedule[index as usize];
            // Freight: seed the dwell state. The platform id is validated, so the
            // arrival edge always resolves.
            let stop = e.stop.map(|s| PendingStop {
                platform: s.platform,
                arrival_edge: self.platform_arrival[&s.platform],
                dwell_total: s.dwell,
                dwell_remaining: s.dwell,
                done: false,
            });
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
                stop,
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

    // --- Phase 2 -----------------------------------------------------------

    /// Trains already standing at a signal request clearance in
    /// (waiting_since, id) order — first come wins, ties go to the lower id.
    fn phase_signal_claims(&mut self, tick: &mut TickState) {
        // Priority first, then first-come, then lowest id. A standing train's
        // head edge IS its signal edge, so `effective_priority` returns that
        // signal's priority without walking. Priority 0 (the default) reduces
        // the key to the historical `(waiting_since, id)` order — bit-identical.
        let mut order: Vec<(std::cmp::Reverse<i8>, u64, TrainId)> = self
            .trains
            .iter()
            .filter(|t| self.at_signal_end(t))
            .map(|t| {
                (
                    std::cmp::Reverse(self.effective_priority(t)),
                    t.waiting_since.map_or(self.now.0, |w| w.0),
                    t.id,
                )
            })
            .collect();
        order.sort();
        for (_, _, id) in order {
            let train = self.train(id);
            let head = train.head_edge();
            // Arrival edges are handled in movement; a signal there is moot.
            if self.sink_by_arrival.contains_key(&head) {
                continue;
            }
            let Some(next_edge) = self.continuation(train, head) else {
                continue;
            };
            if let Some(grant) = self.clearance(train, head, next_edge, tick) {
                self.apply_grant(grant, id, tick);
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
    /// occupant id; claim/reservation owner as fallback — also when the only
    /// occupant is the train itself but a foreign claim/reservation is the
    /// actual reason the block is busy. `None` for a pure self-jam — that is
    /// no wait-for edge (→ `Stalled`, not `Deadlock`).
    fn holder_of(&self, block: BlockId, train: TrainId, tick: &TickState) -> Option<TrainId> {
        if let Some(owners) = tick.occupancy.get(&block)
            && let Some(other) = owners.iter().copied().find(|&o| o != train)
        {
            return Some(other);
        }
        match (tick.claims.get(&block), self.reservations.get(&block)) {
            (Some(&o), _) if o != train => Some(o),
            (_, Some(&o)) if o != train => Some(o),
            _ => None,
        }
    }

    /// A crossing point is free for `train` when no OTHER train covers it (this
    /// tick) or has reserved it (block/chain grant). Mirrors
    /// [`block_free`](Self::block_free) for the cross-block crossing resource.
    fn point_free(&self, point: NodeId, train: TrainId, tick: &TickState) -> bool {
        tick.point_occupancy
            .get(&point)
            .is_none_or(|s| s.iter().all(|&o| o == train))
            && self
                .point_reservations
                .get(&point)
                .is_none_or(|(o, _)| *o == train)
    }

    /// Holder of a busy crossing point, for the wait-for graph. Mirrors
    /// [`holder_of`](Self::holder_of).
    fn point_holder_of(&self, point: NodeId, train: TrainId, tick: &TickState) -> Option<TrainId> {
        if let Some(owners) = tick.point_occupancy.get(&point)
            && let Some(other) = owners.iter().copied().find(|&o| o != train)
        {
            return Some(other);
        }
        match self.point_reservations.get(&point) {
            Some(&(o, _)) if o != train => Some(o),
            _ => None,
        }
    }

    /// Crossing points the train crosses while travelling within `blocks` from
    /// `first`. Mirrors the block walk so a grant reserves the points it will
    /// foul; the `blocks` bound keeps a block grant to its own block.
    fn route_points(&self, train: &Train, first: EdgeId, blocks: &[BlockId]) -> Vec<NodeId> {
        let mut points = Vec::new();
        let mut current = first;
        for _ in 0..=self.graph.edges.len() {
            if !blocks.contains(&self.graph.blocks.block_of(current)) {
                break;
            }
            let node = self.graph.edge(current).to;
            if self.graph.crossing_nodes.contains(&node) && !points.contains(&node) {
                points.push(node);
            }
            if self.sink_by_arrival.contains_key(&current) {
                break;
            }
            match self.continuation(train, current) {
                Some(next) => current = next,
                None => break,
            }
        }
        points
    }

    /// Records a clearance grant. A block grant only claims its single block for
    /// this tick (rear-end protection); it reserves NO crossing points, so it
    /// does not secure a path through a junction — two block signals at a
    /// crossing can let trains collide. A chain grant reserves its whole route's
    /// blocks AND crossing points persistently, which is what makes it safe
    /// through a junction: a point sits INSIDE a block, so without a persistent
    /// reservation a perpendicular train grabs it in the gap before the body
    /// arrives (`or_insert` keeps an existing own reservation's `occupied` flag
    /// on an idempotent re-grant).
    fn apply_grant(&mut self, grant: Grant, id: TrainId, tick: &mut TickState) {
        let (blocks_persist, points): (Vec<BlockId>, Vec<NodeId>) = match grant {
            Grant::Block(block) => {
                tick.claims.insert(block, id);
                (Vec::new(), Vec::new())
            }
            Grant::Chain(blocks, points) => (blocks, points),
        };
        for block in blocks_persist {
            self.reservations.insert(block, id);
        }
        for p in points {
            self.point_reservations.entry(p).or_insert((id, false));
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
                // Refuse to drive into a point a train is ON right now, but a
                // block grant does not RESERVE it (see `apply_grant`): a
                // converging perpendicular train is not held back, so they can
                // still meet at the point. That is the junction risk a chain
                // signal removes.
                let points = self.route_points(train, next_edge, &[block]);
                if self.block_free(block, train.id, tick)
                    && points.iter().all(|&p| self.point_free(p, train.id, tick))
                {
                    Some(Grant::Block(block))
                } else {
                    None
                }
            }
            SignalKind::Chain => {
                let blocks = self.chain_route_blocks(train, next_edge);
                let points = self.route_points(train, next_edge, &blocks);
                if blocks.iter().all(|&b| self.block_free(b, train.id, tick))
                    && points.iter().all(|&p| self.point_free(p, train.id, tick))
                {
                    Some(Grant::Chain(blocks, points))
                } else {
                    None
                }
            }
        }
    }

    /// Blocks a chain signal must secure: along the train's
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
        // Higher-priority trains move (and so claim contested blocks) first;
        // ties keep ascending-id order. This is what makes priority decide a
        // merge where two trains reach their signals in the SAME tick — those
        // are resolved here, not in `phase_signal_claims`. With every priority
        // 0 the key collapses to ascending id, the historical order, so the
        // movement (and the replay hash) is unchanged for existing levels.
        let mut order: Vec<(std::cmp::Reverse<i8>, TrainId)> = self
            .trains
            .iter()
            .map(|t| (std::cmp::Reverse(self.effective_priority(t)), t.id))
            .collect();
        order.sort();
        for (_, id) in order {
            if self.outcome.is_some() {
                return;
            }
            self.move_train(id, tick);
        }
    }

    /// Priority that ranks a train for block contention this tick: the
    /// priority of the next signal on its route (the one it is approaching or
    /// standing at), 0 if none lies ahead. Switches are resolved for this
    /// train, so the walk follows its actual path; the edge-count guard bounds
    /// it on rings. O(edges) worst case, run per train per tick — fine at
    /// puzzle scale. // ponytail: cache per tick if a huge level ever shows it.
    fn effective_priority(&self, train: &Train) -> i8 {
        let mut edge = train.head_edge();
        for _ in 0..=self.graph.edges.len() {
            if let Some(sig) = self.graph.edge(edge).signal {
                return self.graph.signals[sig.0 as usize].priority;
            }
            match self.continuation(train, edge) {
                Some(next) => edge = next,
                None => return 0,
            }
        }
        0
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
            // arrival tick is "head reaches the anchor", not
            // "head rests on the anchor one tick later".
            // Arrival beats everything else.
            if let Some(&sink) = self.sink_by_arrival.get(&head) {
                self.arrive(id, sink);
                return;
            }

            // Freight dwell: the first time the head rests on the assigned
            // platform anchor, hold on free track (occupying the block) for the
            // dwell duration, then resume. A held tick counts as progress so the
            // stall fallback never mistakes a legitimate dwell for a jam.
            if self.dwell_tick(id, head) {
                tick.progressed = true;
                return;
            }

            let Some(next_edge) = self.continuation(self.train(id), head) else {
                let blame = self.blame(self.train(id));
                self.finish(Outcome::Misrouting {
                    train: id,
                    reached: None,
                    blame,
                });
                return;
            };

            if self.graph.edge(head).signal.is_some() {
                match self.clearance(self.train(id), head, next_edge, tick) {
                    Some(grant) => self.apply_grant(grant, id, tick),
                    None => {
                        // Blocked: remember since when (first-come priority)
                        // and on whom we wait (deadlock detection).
                        if self.train(id).waiting_since.is_none() {
                            self.train_mut(id).waiting_since = Some(self.now);
                            self.events.push(SimEvent::SignalBlocked { train: id });
                        }
                        let needed = self.needed_blocks(self.train(id), head, next_edge);
                        let block_holder =
                            needed.iter().find_map(|&b| self.holder_of(b, id, tick));
                        let holder = block_holder.or_else(|| {
                            // No block holder: maybe a crossing point we'd foul.
                            self.route_points(self.train(id), next_edge, &needed)
                                .iter()
                                .find_map(|&p| self.point_holder_of(p, id, tick))
                        });
                        if let Some(holder) = holder {
                            tick.waits.insert(id, holder);
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
            // Entering a crossing point: occupancy takes over this tick too, so a
            // lower-priority train moving later sees it busy.
            let crossed = self.graph.edge(head).to;
            if self.graph.crossing_nodes.contains(&crossed) {
                tick.point_occupancy.entry(crossed).or_default().insert(id);
            }
            let train = self.train_mut(id);
            train.waiting_since = None;
            train.path.push_back(next_edge);
            train.head_dist = Len(0);
        }
        let index = self.index_of(id);
        self.trains[index].trim_path(&self.graph);
    }

    /// Freight dwell tick. `head` is the train's head edge, known to be at its
    /// end (no sink). If the train has an unfinished stop anchored here, count
    /// down one tick and return `true` while it must keep holding; the tick the
    /// dwell reaches zero it flips `done` and returns `false` so the train
    /// crosses this same tick. Deliberately never touches `waiting_since`: a
    /// dwell is a stop on free track, not a signal wait, and must not gain
    /// first-come priority or forge a wait-for edge (plan §4).
    fn dwell_tick(&mut self, id: TrainId, head: EdgeId) -> bool {
        let Some(stop) = self.train(id).stop else {
            return false;
        };
        if stop.done || stop.arrival_edge != head {
            return false;
        }
        let s = self.train_mut(id).stop.as_mut().expect("checked above");
        if s.dwell_remaining.0 > 0 {
            s.dwell_remaining.0 -= 1;
            true
        } else {
            s.done = true;
            false
        }
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
        if sink != self.train(id).sink {
            let blame = self.blame(self.train(id));
            self.finish(Outcome::Misrouting {
                train: id,
                reached: Some(sink),
                blame,
            });
            return;
        }
        // Freight gating: the correct sink only accepts the train once its
        // mandatory unload stop is done. Reaching the sink first means the
        // route bypassed the platform.
        if let Some(stop) = self.train(id).stop
            && !stop.done
        {
            self.finish(Outcome::FreightNotDelivered {
                train: id,
                platform: stop.platform,
            });
            return;
        }
        let due = self.train(id).due;
        self.arrivals.push((id, self.now));
        self.lateness_total += self.now.0.saturating_sub(due.0);
        self.reservations.retain(|_, owner| *owner != id);
        self.point_reservations.retain(|_, (owner, _)| *owner != id);
        self.trains.retain(|t| t.id != id);
        self.events.push(SimEvent::TrainArrived {
            train: id,
            at: self.now,
        });
    }

    /// Last passed switch whose *other* branch would have reached the
    /// target sink (walking backwards).
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
        // Collision (1): strict interval overlap on the same canonical edge —
        // head-on / rear-end on one track.
        let mut per_edge: BTreeMap<EdgeId, Vec<(TrainId, i64, i64)>> = BTreeMap::new();
        // Collision (2): two bodies meeting at a shared interior joint — the
        // crossing point two routes share as a NODE but never as the same edge
        // (lifts the old M0 "crossings don't collide" limitation). A train's
        // strictly-interior nodes are the joints between its consecutive occupied
        // edges; the head tip and tail end are excluded, so a nose-to-tail touch
        // at a node does NOT count — same strict semantics as the edge overlap.
        let mut per_node: BTreeMap<NodeId, Vec<(TrainId, EdgeId)>> = BTreeMap::new();
        let mut occ_buf = Vec::new();
        for train in &self.trains {
            train.occupied_into(&self.graph, &mut occ_buf);
            for &(edge, lo, hi) in &occ_buf {
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
            // `occ_buf` is head-first with no gaps, so each adjacent pair shares
            // a node strictly inside the body: the `from` of the head-side edge.
            for pair in occ_buf.windows(2) {
                let node = self.graph.edge(pair[0].0).from;
                per_node.entry(node).or_default().push((train.id, pair[0].0));
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
        for occupants in per_node.values() {
            for (i, &(id_a, edge_a)) in occupants.iter().enumerate() {
                for &(id_b, _) in &occupants[i + 1..] {
                    if id_a != id_b {
                        let pair = (id_a.min(id_b), id_a.max(id_b));
                        self.finish(Outcome::Collision {
                            trains: pair,
                            edge: edge_a,
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
    ///
    /// Deliberate exception: `passed_switches` (a savegame stores it for
    /// misrouting blame) is *not* written. It grows with every switch
    /// crossing and is never trimmed, so hashing it would cost
    /// O(total crossings) per tick — quadratic over a run. It adds no
    /// commitment either: each crossing pushes the taken edge onto `path`,
    /// which IS hashed in the same tick, and the per-tick chain in
    /// [`Self::advance_hash`] commits to that history permanently.
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
            // Freight dwell state is real mutable state (a savegame stores it),
            // so it must commit to the hash or replays/codes drift. Written only
            // for freight trains: a train's stop is fixed at spawn and never
            // becomes/unbecomes `None` mid-run, so passenger trains contribute no
            // bytes and every pre-freight replay hash stays byte-identical.
            if let Some(s) = t.stop {
                h.write_u32(s.arrival_edge.0);
                h.write_u64(s.dwell_remaining.0);
                h.write_u32(u32::from(s.done));
            }
            h.write_u32(t.path.len() as u32);
            for e in &t.path {
                h.write_u32(e.0);
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

    /// `trains` is always sorted by ascending id — lookups binary-search.
    fn index_of(&self, id: TrainId) -> usize {
        self.trains
            .binary_search_by_key(&id, |t| t.id)
            .expect("caller guarantees the train is alive")
    }

    fn train(&self, id: TrainId) -> &Train {
        &self.trains[self.index_of(id)]
    }

    fn train_mut(&mut self, id: TrainId) -> &mut Train {
        let index = self.index_of(id);
        &mut self.trains[index]
    }
}

enum Grant {
    /// One block, claimed per-tick. No crossing points: a block grant does not
    /// secure a path through a junction.
    Block(BlockId),
    /// A chain route's blocks plus its crossing points (reserved persistently).
    Chain(Vec<BlockId>, Vec<NodeId>),
}

/// Default slack budget for [`suggest_dues`]: a percentage of each train's run
/// time. A modest band so a "good enough" solution still meets the timetable,
/// rather than demanding the reference's exact timing.
pub const DUE_SLACK_PCT: u64 = 10;

/// Per-schedule-entry `due` ticks that make `solution` punctual: each train's
/// measured arrival in `solution`, plus `slack_pct`% of its run time (so longer
/// runs get proportionally more leeway). Setting the level's `due` to these
/// makes 0 lateness achievable, with `solution` as the reference timetable.
///
/// Result is in schedule order (one per `level.schedule` entry). `Err` when
/// `solution` is invalid or does not finish as `Success` — without a clean run
/// there are no arrivals to measure, so the caller keeps the existing `due`.
pub fn suggest_dues(level: &Level, solution: &Layout, slack_pct: u64) -> Result<Vec<Tick>, String> {
    let mut sim = Sim::new(level, solution).map_err(|_| "Lösung validiert nicht".to_string())?;
    match sim.run(Tick(50_000)) {
        Outcome::Success { .. } => {}
        other => return Err(format!("Lösung endet nicht mit Erfolg ({other:?})")),
    }
    let arrivals = sim.arrivals();
    level
        .schedule
        .iter()
        .map(|entry| {
            let (_, arrival) = arrivals
                .iter()
                .find(|(t, _)| *t == entry.train)
                .ok_or_else(|| format!("Zug {} kam nicht an", entry.train.0))?;
            let journey = arrival.0.saturating_sub(entry.depart.0);
            let slack = (journey * slack_pct).div_ceil(100);
            Ok(Tick(arrival.0 + slack))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Dir8;
    use crate::layout::{SignalDef, SwitchDef, TrackPiece};
    use crate::level::{Par, PlatformDef, PlatformStop, ScheduleEntry, SinkDef, SourceDef};
    use crate::units::{PlatformId, Speed, TrainClass};

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
                    priority: 0,
                },
                SignalDef {
                    cell: cell(0, -1),
                    at: Dir8::NE,
                    kind: SignalKind::Block,
                    priority: 0,
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
                    label: String::new(),
                },
                SourceDef {
                    id: SourceId(1),
                    cell: cell(0, -1),
                    dir: Dir8::W,
                    label: String::new(),
                },
            ],
            sinks: vec![SinkDef {
                id: SinkId(0),
                cell: cell(2, 0),
                dir: Dir8::E,
                label: "OST".into(),
            }],
            platforms: vec![],
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
                    stop: None,
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
                    stop: None,
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

    /// Calibrated dues make the reference solution punctual: applying the
    /// suggested `due` to the schedule, the same solution scores 0 lateness.
    #[test]
    fn suggest_dues_makes_solution_punctual() {
        let (mut level, layout) = merge_level();
        let dues = suggest_dues(&level, &layout, 10).expect("solvable reference");
        assert_eq!(dues.len(), level.schedule.len());
        for (entry, due) in level.schedule.iter_mut().zip(&dues) {
            entry.due = *due;
        }
        let mut sim = Sim::new(&level, &layout).expect("valid");
        match sim.run(Tick(10_000)) {
            Outcome::Success { score } => assert_eq!(score.lateness, 0, "calibrated due ⇒ punctual"),
            other => panic!("got {other:?}"),
        }
    }

    /// Both trains reach their signals in the same tick and want the same
    /// block: the lower id must win, the other must wait.
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

    /// Same merge, same tick — but source 1's approach signal carries a higher
    /// priority. The default lowest-id winner is overruled: t1 crosses, t0
    /// waits. The exact mirror of `same_tick_contention_goes_to_lower_id`,
    /// proving priority drives the `phase_movement` ordering (the contention is
    /// resolved there, not in `phase_signal_claims`, for same-tick arrivals).
    #[test]
    fn higher_priority_signal_wins_same_tick_contention() {
        let (level, mut layout) = merge_level();
        layout
            .signals
            .iter_mut()
            .find(|s| s.cell == cell(0, -1))
            .expect("source-1 signal exists")
            .priority = 1;

        let mut sim = Sim::new(&level, &layout).expect("valid");
        for _ in 0..13 {
            sim.step();
        }
        let t0 = &sim.trains()[0];
        let t1 = &sim.trains()[1];
        assert_eq!(t1.path.len(), 2, "t1 (higher priority) crossed the signal");
        assert_eq!(t1.head_dist, Len(93));
        assert_eq!(t0.path.len(), 1, "t0 held at its signal");
        assert_eq!(t0.head_dist, Len(707));
        assert!(t0.waiting_since.is_some(), "t0 is registered as waiting");

        let outcome = sim.run(Tick(10_000));
        assert!(matches!(outcome, Outcome::Success { .. }), "got {outcome:?}");
    }

    /// Straight W–E line of `n` cells: source (0,0)W, sink (n-1,0)E, no
    /// platforms/schedule (the caller adds them).
    fn straight(n: i32) -> (Level, Layout) {
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
            name: "line".into(),
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
            platforms: vec![],
            schedule: vec![],
            par: Par {
                throughput: Tick(0),
                material: 0,
                lateness: 0,
            },
        };
        (level, layout)
    }

    fn freight_entry(dwell: u64, platform: PlatformId) -> ScheduleEntry {
        ScheduleEntry {
            train: TrainId(0),
            class: TrainClass(0),
            length: Len(400),
            speed: Speed(100),
            source: SourceId(0),
            sink: SinkId(0),
            depart: Tick(0),
            due: Tick(10_000),
            stop: Some(PlatformStop {
                platform,
                dwell: Tick(dwell),
            }),
        }
    }

    /// A freight train that crosses its platform dwells there, then arrives.
    /// The dwell lengthens the run by (at least) the dwell duration — the same
    /// line without a stop finishes earlier.
    #[test]
    fn freight_dwells_then_arrives() {
        let (mut level, layout) = straight(5);
        // Platform on the eastbound walk: (2,0)'s E connector.
        level.platforms = vec![PlatformDef {
            id: PlatformId(0),
            cell: cell(2, 0),
            dir: Dir8::E,
            label: "RAMPE".into(),
        }];
        level.schedule = vec![freight_entry(30, PlatformId(0))];

        let mut sim = Sim::new(&level, &layout).expect("valid");
        let outcome = sim.run(Tick(10_000));
        assert!(matches!(outcome, Outcome::Success { .. }), "got {outcome:?}");
        let freight_arrival = sim.arrivals()[0].1;

        // Baseline: same line, same train, but no stop.
        let (mut base_level, base_layout) = straight(5);
        base_level.schedule = vec![{
            let mut e = freight_entry(30, PlatformId(0));
            e.stop = None;
            e
        }];
        let mut base = Sim::new(&base_level, &base_layout).expect("valid");
        assert!(matches!(base.run(Tick(10_000)), Outcome::Success { .. }));
        let base_arrival = base.arrivals()[0].1;

        // The stop delays arrival by ~dwell ticks (a one-tick partial-movement
        // artifact at the platform edge makes it dwell-1 here — deterministic).
        assert!(
            freight_arrival.0 >= base_arrival.0 + 29,
            "dwell (30) must delay arrival by ~30: freight {} vs baseline {}",
            freight_arrival.0,
            base_arrival.0
        );
    }

    /// A freight train whose route never crosses its assigned platform reaches
    /// the sink undelivered — a distinct failure, not a silent success.
    #[test]
    fn freight_bypassing_platform_is_not_delivered() {
        let (mut level, layout) = straight(5);
        // Platform anchored at the entry connector (0,0)W: on track, but an
        // eastbound run never traverses center→W, so the dwell never triggers.
        level.platforms = vec![PlatformDef {
            id: PlatformId(0),
            cell: cell(0, 0),
            dir: Dir8::W,
            label: "GEGEN".into(),
        }];
        level.schedule = vec![freight_entry(30, PlatformId(0))];

        let mut sim = Sim::new(&level, &layout).expect("valid");
        let outcome = sim.run(Tick(10_000));
        assert_eq!(
            outcome,
            Outcome::FreightNotDelivered {
                train: TrainId(0),
                platform: PlatformId(0),
            },
            "reached the sink without dwelling at the platform"
        );
    }

    /// A lone dwelling train must not trip the stall fallback: the dwell counts
    /// as progress. With dwell ≫ STALL_TICKS the run would end `Stalled` if the
    /// exemption were missing.
    #[test]
    fn long_dwell_does_not_stall() {
        let (mut level, layout) = straight(5);
        level.platforms = vec![PlatformDef {
            id: PlatformId(0),
            cell: cell(2, 0),
            dir: Dir8::E,
            label: "RAMPE".into(),
        }];
        level.schedule = vec![freight_entry(STALL_TICKS + 50, PlatformId(0))];

        let mut sim = Sim::new(&level, &layout).expect("valid");
        let outcome = sim.run(Tick(STALL_TICKS * 3));
        assert!(
            matches!(outcome, Outcome::Success { .. }),
            "a legitimate dwell is not a stall: {outcome:?}"
        );
    }
}
