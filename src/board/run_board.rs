//! Run/Result rendering.
//!
//! Performance contract: the board
//! geometry never changes during a run, so it is spawned ONCE on entering
//! `Run`. Per frame only the dynamic state is mutated **in place**:
//! - block bands recolour / resize by occupancy ([`update_run_board`]),
//! - signal lamps recolour by their precomputed gated block,
//! - trains (few, and moving) are the only sprites still respawned per frame
//!   ([`redraw_trains`]).
//!
//! This replaced a full despawn+respawn of every sprite every frame — the
//! dominant cost on large layouts.

use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use stellwerk_sim::Sim;
use stellwerk_sim::graph::{Next, TrackGraph};
use stellwerk_sim::grid::{Cell, Dir8, Point};
use stellwerk_sim::layout::SignalKind;
use stellwerk_sim::units::{BlockId, EdgeId};

use super::draw::{Tag, band, draw_stations, lamp, label, signal_arrow, signal_pos};
use super::geometry::{CELL, cell_world, point_world};
use super::palette::*;
use crate::font::UiFont;
use crate::run::RunCtl;
use crate::state::{ActiveLevel, GameState};

/// Track band of one block — its colour/width is recoloured in place by
/// [`update_run_board`] instead of being respawned each frame.
#[derive(Component)]
pub(super) struct BlockBand(BlockId);

/// Signal lamp (and its direction tick). The block the signal protects is
/// baked in at spawn, so the per-frame recolour needs no graph walk and no
/// per-frame `(point, point) → edge` map.
/// `None` = always green (dead end or no gated edge).
#[derive(Component, Clone, Copy)]
pub(super) struct SignalLamp {
    next_block: Option<BlockId>,
}

/// Per-frame train sprites — the only group still despawned + respawned each
/// frame (trains are few and actually move).
#[derive(Component)]
pub(super) struct TrainGfx;

// --- Static board: spawned once per run ----------------------------------------

/// Spawns the static geometry the first frame of a run. Runs in `Update`, not
/// `OnEnter(Run)`: `RunCtl` is inserted via deferred commands during the same
/// `OnEnter` (by `run::start_run`), so it is not yet visible to OnEnter
/// systems. The `BlockBand` guard makes this a one-shot per run (cleanup on
/// leaving Run despawns the markers, so the next run rebuilds).
pub(super) fn spawn_run_board_static(
    mut commands: Commands,
    ui_font: Res<UiFont>,
    existing: Query<(), With<BlockBand>>,
    active: Option<Res<ActiveLevel>>,
    ctl: Option<Res<RunCtl>>,
) {
    if !existing.is_empty() {
        return; // already built for this run
    }
    let (Some(active), Some(ctl)) = (active, ctl) else {
        return;
    };
    let font = ui_font.0.clone();
    let sim = &ctl.sim;
    let graph = sim.graph();
    let (occupied, reserved) = block_states(sim);

    // Buildable grid + stations (purely static).
    for cell in &active.level.buildable {
        let mut entity = commands.spawn((
            Sprite::from_color(col_grid(), Vec2::splat(CELL - 4.0)),
            Transform::from_translation(cell_world(*cell).extend(0.0)),
        ));
        Tag::Live.apply(&mut entity);
    }
    draw_stations(&mut commands, &font, &active.level, Tag::Live);

    // One band per canonical edge, tagged with its block for live recolouring.
    for (i, edge) in graph.edges.iter().enumerate() {
        if edge.opposite.0 < i as u32 {
            continue; // canonical direction only
        }
        let block = graph.blocks.block_of(EdgeId(i as u32));
        let (color, width) = band_style(block, &occupied, &reserved);
        let entity = band(
            &mut commands,
            point_world(graph.node(edge.from).point),
            point_world(graph.node(edge.to).point),
            width,
            color,
            2.0,
            Tag::Live,
        );
        commands.entity(entity).insert(BlockBand(block));
    }

    // Signal lamps: resolve each signal's protected block ONCE, here.
    let edge_at = edge_lookup(graph);
    let merged = active.level.fixed.merged(&ctl.layout);
    for signal in &merged.signals {
        let next_block = signal_next_block(graph, &edge_at, signal.cell, signal.at);
        let marker = SignalLamp { next_block };
        let color = lamp_color(next_block, &occupied, &reserved);
        let lamp_e = lamp(
            &mut commands,
            signal_pos(signal.cell, signal.at),
            14.0,
            color,
            matches!(signal.kind, SignalKind::Chain),
            5.0,
            Tag::Live,
        );
        commands.entity(lamp_e).insert(marker);
        for arrow_e in signal_arrow(&mut commands, signal.cell, signal.at, color, Tag::Live) {
            commands.entity(arrow_e).insert(marker);
        }
    }
}

// --- Per-frame state update (in place, no spawn/despawn) ------------------------

pub(super) fn update_run_board(
    ctl: Option<Res<RunCtl>>,
    state: Res<State<GameState>>,
    mut bands: Query<(&BlockBand, &mut Sprite), Without<SignalLamp>>,
    mut lamps: Query<(&SignalLamp, &mut Sprite), Without<BlockBand>>,
    mut commands: Commands,
) {
    let Some(ctl) = ctl else {
        return;
    };
    let (occupied, reserved) = block_states(&ctl.sim);
    for (band, mut sprite) in &mut bands {
        let (color, width) = band_style(band.0, &occupied, &reserved);
        sprite.color = color;
        if let Some(size) = sprite.custom_size.as_mut() {
            size.y = width;
        }
    }
    // A signal "switches" when its lamp changes aspect (green↔red). Each signal
    // owns several SignalLamp sprites (lamp + arrow); fire ONE sound per frame
    // no matter how many lamps/sprites flipped, like `button_click_sfx`.
    let mut switched = false;
    for (signal, mut sprite) in &mut lamps {
        let color = lamp_color(signal.next_block, &occupied, &reserved);
        switched |= sprite.color != color;
        sprite.color = color;
    }
    // Only sound a flip during live play — the once-off recolour on entering
    // Result must stay silent (a final-tick flip would otherwise click on the
    // outcome screen).
    if switched && *state.get() == GameState::Run {
        commands.trigger(crate::audio::SfxKind::Signal);
    }
}

pub(super) fn redraw_trains(
    mut commands: Commands,
    ui_font: Res<UiFont>,
    existing: Query<Entity, With<TrainGfx>>,
    ctl: Option<Res<RunCtl>>,
) {
    let Some(ctl) = ctl else {
        return;
    };
    for e in &existing {
        commands.entity(e).despawn();
    }
    let font = ui_font.0.clone();
    let sim = &ctl.sim;
    let graph = sim.graph();
    let mut buf = Vec::new();
    for train in sim.trains() {
        train.occupied_into(graph, &mut buf);
        for &(edge, lo, hi) in &buf {
            let data = graph.edge(edge);
            let a = point_world(graph.node(data.from).point);
            let b = point_world(graph.node(data.to).point);
            let len = data.len.0 as f32;
            let entity = band(
                &mut commands,
                a.lerp(b, lo.0 as f32 / len),
                a.lerp(b, hi.0 as f32 / len),
                9.0,
                col_train(),
                10.0,
                Tag::Live,
            );
            commands.entity(entity).insert(TrainGfx);
        }
        let head = ctl.interpolated_head(train.id);
        let lamp_e = lamp(&mut commands, head, 13.0, col_head(), false, 11.0, Tag::Live);
        commands.entity(lamp_e).insert(TrainGfx);
        let label_e = label(
            &mut commands,
            &font,
            head + Vec2::new(0.0, 20.0),
            format!("{}", train.id.0),
            13.0,
            col_label(),
            Tag::Live,
        );
        commands.entity(label_e).insert(TrainGfx);
    }
}

// --- Shared state helpers ------------------------------------------------------

/// Blocks currently occupied by a train body / head, and blocks reserved by a
/// chain signal. Reuses one buffer across all trains (no per-train alloc).
fn block_states(sim: &Sim) -> (BTreeSet<BlockId>, BTreeSet<BlockId>) {
    let graph = sim.graph();
    let mut occupied: BTreeSet<BlockId> = BTreeSet::new();
    let mut buf = Vec::new();
    for train in sim.trains() {
        train.occupied_into(graph, &mut buf);
        for &(edge, lo, hi) in &buf {
            if hi > lo {
                occupied.insert(graph.blocks.block_of(edge));
            }
        }
        occupied.insert(graph.blocks.block_of(train.head_edge()));
    }
    let reserved: BTreeSet<BlockId> = sim.reservations().keys().copied().collect();
    (occupied, reserved)
}

/// Colour + width of a block band by state (colour AND shape — never colour
/// alone, for accessibility).
fn band_style(block: BlockId, occupied: &BTreeSet<BlockId>, reserved: &BTreeSet<BlockId>) -> (Color, f32) {
    if occupied.contains(&block) {
        (col_occupied(), 9.0)
    } else if reserved.contains(&block) {
        (col_reserved(), 5.0)
    } else {
        // Idle: a distinct hue per block so the signal-cut partition is
        // visible at rest, not just when a train lights one up.
        (col_block(block), 7.0)
    }
}

/// Green unless the signal's protected block is occupied or reserved.
fn lamp_color(
    next_block: Option<BlockId>,
    occupied: &BTreeSet<BlockId>,
    reserved: &BTreeSet<BlockId>,
) -> Color {
    let lit = next_block.is_none_or(|b| !(occupied.contains(&b) || reserved.contains(&b)));
    if lit {
        col_signal_green()
    } else {
        col_signal_red()
    }
}

/// `(from point, to point) → edge id` over the whole graph. Built once at run
/// start, not per frame.
fn edge_lookup(graph: &TrackGraph) -> BTreeMap<(Point, Point), EdgeId> {
    let mut map = BTreeMap::new();
    for (i, edge) in graph.edges.iter().enumerate() {
        map.insert(
            (graph.node(edge.from).point, graph.node(edge.to).point),
            EdgeId(i as u32),
        );
    }
    map
}

/// The block a signal protects: walk its gated edge → continuation (switches
/// resolved to the default branch, like the live display heuristic). `None`
/// for a dead end or a signal whose gated edge is missing (→ always green).
fn signal_next_block(
    graph: &TrackGraph,
    edge_at: &BTreeMap<(Point, Point), EdgeId>,
    cell: Cell,
    at: Dir8,
) -> Option<BlockId> {
    let gated = *edge_at.get(&(cell.center_point(), cell.connector_point(at)))?;
    let next = match graph.edge(gated).next {
        Next::Fixed(e) => e,
        Next::SwitchChoice { switch } => {
            let sw = &graph.switches[switch as usize];
            sw.branch_out[sw.default_branch as usize]
        }
        Next::DeadEnd => return None,
    };
    Some(graph.blocks.block_of(next))
}
