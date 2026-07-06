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
use std::collections::{BTreeMap, BTreeSet, HashMap};
use stellwerk_sim::Sim;
use stellwerk_sim::graph::{Next, TrackGraph};
use stellwerk_sim::grid::{Cell, Dir8, Point};
use stellwerk_sim::layout::SignalKind;
use stellwerk_sim::units::{BlockId, EdgeId, TrainId};

use super::draw::{Tag, band, draw_blocks, draw_stations, lamp, label, signal_arrow, signal_pos};
use super::geometry::{CELL, cell_world, point_world};
use super::palette::*;
use crate::font::UiFont;
use crate::run::RunCtl;
use crate::state::{ActiveLevel, GameState};

/// Track band of one block — its colour/width is recoloured in place by
/// [`update_run_board`] instead of being respawned each frame.
#[derive(Component)]
pub(super) struct BlockBand {
    block: BlockId,
    /// Was this block reserved last frame? The free→reserved edge fires the
    /// route-formation glow pulse.
    reserved_was: bool,
    /// Seconds of glow remaining for that pulse (0 = none).
    pulse: f32,
}

/// Route-formation glow: how long the pulse lasts and how much it brightens.
const PULSE_SECS: f32 = 0.45;
const PULSE_BOOST: f32 = 0.6;

// Train body layout, all in LE (1000 LE = one cell). A train is a locomotive
// plus N wagons, with a coupling between every unit: Loco · [coupling · Wagon]×N.
// The sim only sees `length` = LOCO_LEN + N·SLOT.
const LOCO_LEN: f32 = 200.0;
const WAGON_LEN: f32 = 200.0;
const COUPLING_LEN: f32 = 20.0;
const SLOT: f32 = WAGON_LEN + COUPLING_LEN;
// Band widths (px) — the loco a tick wider than a wagon, the coupling thinner.
const LOCO_W: f32 = 11.0;
const WAGON_W: f32 = 9.0;
const COUPLING_W: f32 = 6.0;
const CARGO_W: f32 = 5.0;
/// LE trimmed off each end of a freight wagon for the inset cargo fill.
const CARGO_INSET: f32 = 25.0;

/// Draws the body sub-interval `[d0, d1]` (LE from the head) as one or more
/// bands. `segs` is the body polyline head-first: `(head-near point, tail-near
/// point, segment length LE)`. Clips the interval per segment, so a slice may
/// span several track edges/curves.
fn body_slice(
    commands: &mut Commands,
    segs: &[(Vec2, Vec2, f32)],
    d0: f32,
    d1: f32,
    width: f32,
    color: Color,
    z: f32,
) {
    let mut cursor = 0.0;
    for &(head_pt, tail_pt, len) in segs {
        let (seg_start, seg_end) = (cursor, cursor + len);
        cursor = seg_end;
        if len <= 0.0 {
            continue;
        }
        let lo = d0.max(seg_start);
        let hi = d1.min(seg_end);
        if hi > lo {
            let a = head_pt.lerp(tail_pt, (lo - seg_start) / len);
            let b = head_pt.lerp(tail_pt, (hi - seg_start) / len);
            let ent = band(commands, a, b, width, color, z, Tag::Live);
            commands.entity(ent).insert(TrainGfx);
        }
    }
}

/// Scales a colour's linear channels up by `k` — a transient brightening for
/// the route-formation pulse. Works with bloom on (more bloom) or off
/// (the palette is already tonemapped into gamut).
fn brighten(color: Color, k: f32) -> Color {
    let c = color.to_linear();
    Color::LinearRgba(LinearRgba::rgb(
        c.red * (1.0 + k),
        c.green * (1.0 + k),
        c.blue * (1.0 + k),
    ))
}

/// Signal lamp (and its direction tick). The block the signal protects is
/// baked in at spawn, so the per-frame recolour needs no graph walk and no
/// per-frame `(point, point) → edge` map.
/// `None` = always green (dead end or no gated edge).
#[derive(Component, Clone, Copy)]
pub(super) struct SignalLamp {
    next_block: Option<BlockId>,
}

/// The colour-blind stop bar across a red signal (see [`stop_bar_color`]). Also
/// carries [`SignalLamp`], but is updated separately so the lamp recolour does
/// not turn the bar green — it only ever shows red or vanishes.
#[derive(Component)]
pub(super) struct StopBar;

/// Query filters split out to keep `update_run_board` under clippy's
/// type-complexity threshold (same trick as `widgets::ChangedButton`).
type LampOnly = (Without<BlockBand>, Without<StopBar>);
type StopBarOnly = (Without<BlockBand>, With<StopBar>);

/// Per-frame train body sprites (bands + head lamp) — plain quads, despawned
/// and respawned each frame (trains are few, and the segment count changes as
/// a train grows/moves, so pooling them would need bookkeeping for little
/// gain). The text labels are NOT in this group — see [`TrainLabel`].
#[derive(Component)]
pub(super) struct TrainGfx;

/// Retained per-train number label. `Text2d` is expensive to spawn (full glyph
/// layout + atlas work every time), so unlike the body sprites these are kept
/// across frames: spawned once when a train appears, only their `Transform` is
/// moved per frame, despawned when the train leaves.
#[derive(Component)]
pub(super) struct TrainLabel(TrainId);

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
    // Sandbox blocks (non-buildable holes); campaign gaps are authored shape.
    if active.sandbox {
        draw_blocks(&mut commands, &active.level.buildable, Tag::Live);
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
        commands.entity(entity).insert(BlockBand {
            block,
            reserved_was: reserved.contains(&block),
            pulse: 0.0,
        });
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
        // Stop bar across the lamp: a non-colour cue for red (GDD §9).
        let pos = signal_pos(signal.cell, signal.at);
        let bar = band(
            &mut commands,
            pos - Vec2::new(9.0, 0.0),
            pos + Vec2::new(9.0, 0.0),
            4.0,
            stop_bar_color(next_block, &occupied, &reserved),
            5.5,
            Tag::Live,
        );
        commands.entity(bar).insert((marker, StopBar));
    }
}

// --- Per-frame state update (in place, no spawn/despawn) ------------------------

pub(super) fn update_run_board(
    time: Res<Time>,
    ctl: Option<Res<RunCtl>>,
    state: Res<State<GameState>>,
    mut bands: Query<(&mut BlockBand, &mut Sprite), Without<SignalLamp>>,
    mut lamps: Query<(&SignalLamp, &mut Sprite), LampOnly>,
    mut stopbars: Query<(&SignalLamp, &mut Sprite), StopBarOnly>,
    mut commands: Commands,
) {
    let Some(ctl) = ctl else {
        return;
    };
    let (occupied, reserved) = block_states(&ctl.sim);
    let dt = time.delta_secs();
    for (mut band, mut sprite) in &mut bands {
        // Pulse on the free→reserved transition (route just formed).
        let is_reserved = reserved.contains(&band.block);
        if is_reserved && !band.reserved_was {
            band.pulse = PULSE_SECS;
        }
        band.reserved_was = is_reserved;

        let (mut color, width) = band_style(band.block, &occupied, &reserved);
        if band.pulse > 0.0 {
            band.pulse = (band.pulse - dt).max(0.0);
            color = brighten(color, (band.pulse / PULSE_SECS) * PULSE_BOOST);
        }
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
    // Stop bars: show red across a red signal, vanish when it goes green.
    for (signal, mut sprite) in &mut stopbars {
        sprite.color = stop_bar_color(signal.next_block, &occupied, &reserved);
    }
}

pub(super) fn redraw_trains(
    mut commands: Commands,
    ui_font: Res<UiFont>,
    bodies: Query<Entity, With<TrainGfx>>,
    mut labels: Query<(Entity, &TrainLabel, &mut Transform)>,
    mut gizmos: Gizmos,
    ctl: Option<Res<RunCtl>>,
) {
    let Some(ctl) = ctl else {
        return;
    };
    let sim = &ctl.sim;
    let graph = sim.graph();

    // Body bands + head lamp: plain sprites, respawned each frame.
    // ponytail: O(trains × segments) respawn each frame; pool by TrainId if a
    // level ever runs enough trains for this to show in the profile.
    for e in &bodies {
        commands.entity(e).despawn();
    }
    let mut buf = Vec::new();
    let mut segs: Vec<(Vec2, Vec2, f32)> = Vec::new();
    for train in sim.trains() {
        let class = train.class;
        let loco_col = col_class_loco(class);
        // Body polyline head-first: `hi` is the head-near end of each edge (the
        // train travels from→to, head toward `to`).
        train.occupied_into(graph, &mut buf);
        segs.clear();
        for &(edge, lo, hi) in &buf {
            let data = graph.edge(edge);
            let wf = point_world(graph.node(data.from).point);
            let wt = point_world(graph.node(data.to).point);
            let len = data.len.0 as f32;
            if len <= 0.0 {
                continue;
            }
            let head_pt = wf.lerp(wt, hi.0 as f32 / len);
            let tail_pt = wf.lerp(wt, lo.0 as f32 / len);
            segs.push((head_pt, tail_pt, (hi.0 - lo.0) as f32));
        }
        // Loco (bright front), then each wagon behind its darker coupling. A slice
        // past the tail draws nothing, so a partial last wagon clips cleanly.
        body_slice(&mut commands, &segs, 0.0, LOCO_LEN, LOCO_W, loco_col, 10.1);
        let wagon_col = col_class_wagon(class);
        let wagons = (((train.length.0 - LOCO_LEN as i64).max(0)) as f32 / SLOT).round() as usize;
        for i in 0..wagons {
            let base = LOCO_LEN + i as f32 * SLOT;
            body_slice(&mut commands, &segs, base, base + COUPLING_LEN, COUPLING_W, col_coupling(), 10.0);
            body_slice(&mut commands, &segs, base + COUPLING_LEN, base + SLOT, WAGON_W, wagon_col, 10.0);
            // Freight (class 1): inset grey cargo → open gondola look.
            if class.0 == 1 {
                body_slice(
                    &mut commands,
                    &segs,
                    base + COUPLING_LEN + CARGO_INSET,
                    base + SLOT - CARGO_INSET,
                    CARGO_W,
                    col_cargo(),
                    10.3,
                );
            }
        }
        // Loco head marker (class silhouette) at the interpolated head — the
        // second, colour-blind-safe class cue. 0 square · 1 diamond · 2 chevron.
        let head = ctl.interpolated_head(train.id);
        match class.0 {
            1 => {
                let e = lamp(&mut commands, head, 13.0, loco_col, true, 11.0, Tag::Live);
                commands.entity(e).insert(TrainGfx);
            }
            2 => {
                let d = graph.edge(train.head_edge());
                let dir = (point_world(graph.node(d.to).point)
                    - point_world(graph.node(d.from).point))
                .normalize_or_zero();
                let perp = dir.perp();
                let back = head - dir * 9.0;
                for e in [
                    band(&mut commands, head, back + perp * 7.2, 3.0, loco_col, 11.0, Tag::Live),
                    band(&mut commands, head, back - perp * 7.2, 3.0, loco_col, 11.0, Tag::Live),
                ] {
                    commands.entity(e).insert(TrainGfx);
                }
            }
            _ => {
                let e = lamp(&mut commands, head, 13.0, loco_col, false, 11.0, Tag::Live);
                commands.entity(e).insert(TrainGfx);
            }
        }

        // Dwell timer: while a freight train unloads at its platform, a yellow
        // disc sits on the head and empties clockwise from 12 o'clock as the stop
        // counts down; the eaten wedge is transparent (reveals the board). Drawn
        // as a gizmo fan of radii — renders on top, no per-frame sprite churn.
        // ponytail: steps per tick (10 Hz); smooth via the tick fraction if it
        // ever looks choppy.
        if let Some(stop) = train.stop
            && !stop.done
            && stop.dwell_total.0 > 0
            && train.head_edge() == stop.arrival_edge
            && train.head_dist == graph.edge(stop.arrival_edge).len
        {
            use std::f32::consts::{FRAC_PI_2, TAU};
            let f = stop.dwell_remaining.0 as f32 / stop.dwell_total.0 as f32;
            const R: f32 = 11.0;
            // Remaining sector: starts where the (clockwise) eaten wedge ends and
            // sweeps clockwise back to 12 o'clock.
            let start = FRAC_PI_2 - (1.0 - f) * TAU;
            let sweep = f * TAU;
            let segs = ((128.0 * f).ceil() as usize).max(2);
            let col = col_dwell();
            for k in 0..=segs {
                let a = start - (k as f32 / segs as f32) * sweep;
                gizmos.line_2d(head, head + Vec2::from_angle(a) * R, col);
            }
        }
    }

    // Number labels: retained Text2d, only moved (never respawned per frame).
    // Reconcile the live train set against the existing label entities. Relies
    // on TrainId being stable for a train's lifetime (it is: the sim mints ids
    // from the schedule and never recycles them).
    let mut existing: HashMap<TrainId, Entity> =
        labels.iter().map(|(e, l, _)| (l.0, e)).collect();
    for train in sim.trains() {
        // Offset the number to the SIDE of the track (perpendicular to travel),
        // biased upward — so it never lands on the body, which runs along the
        // travel axis (a downward train used to have it sitting in its wagons).
        let d = graph.edge(train.head_edge());
        let dir = (point_world(graph.node(d.to).point) - point_world(graph.node(d.from).point))
            .normalize_or_zero();
        let mut off = dir.perp() * 22.0;
        if off == Vec2::ZERO {
            off = Vec2::new(0.0, 22.0);
        } else if off.y < 0.0 {
            off = -off;
        }
        let pos = ctl.interpolated_head(train.id) + off;
        // z above the body bands (10) and head lamp (11) so the number stays on
        // top of the train.
        const LABEL_Z: f32 = 12.0;
        if let Some(e) = existing.remove(&train.id) {
            if let Ok((_, _, mut tf)) = labels.get_mut(e) {
                tf.translation = pos.extend(LABEL_Z);
            }
        } else {
            let label_e = label(
                &mut commands,
                &ui_font.0,
                pos,
                format!("{}", train.id.0),
                13.0,
                col_label(),
                Tag::Live,
            );
            commands
                .entity(label_e)
                .insert((TrainLabel(train.id), Transform::from_translation(pos.extend(LABEL_Z))));
        }
    }
    // Trains that left the world: drop their labels.
    for e in existing.into_values() {
        commands.entity(e).despawn();
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

/// A signal shows green iff its protected block is free.
fn signal_lit(
    next_block: Option<BlockId>,
    occupied: &BTreeSet<BlockId>,
    reserved: &BTreeSet<BlockId>,
) -> bool {
    next_block.is_none_or(|b| !(occupied.contains(&b) || reserved.contains(&b)))
}

/// Green unless the signal's protected block is occupied or reserved.
fn lamp_color(
    next_block: Option<BlockId>,
    occupied: &BTreeSet<BlockId>,
    reserved: &BTreeSet<BlockId>,
) -> Color {
    if signal_lit(next_block, occupied, reserved) {
        col_signal_green()
    } else {
        col_signal_red()
    }
}

/// Accessibility (GDD §9): a red signal also grows a stop bar — a *shape* cue,
/// not just a hue change. Transparent (invisible) when the signal is green.
fn stop_bar_color(
    next_block: Option<BlockId>,
    occupied: &BTreeSet<BlockId>,
    reserved: &BTreeSet<BlockId>,
) -> Color {
    if signal_lit(next_block, occupied, reserved) {
        Color::NONE
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
