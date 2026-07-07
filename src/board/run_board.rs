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
use stellwerk_sim::graph::{Next, NodeKind, TrackGraph};
use stellwerk_sim::grid::{Cell, Dir8, Point};
use stellwerk_sim::layout::SignalKind;
use stellwerk_sim::units::{BlockId, EdgeId, NodeId, TrainId};

use super::draw::{Tag, band, draw_blocks, draw_stations, lamp, label, signal_arrow, signal_pos};
use super::geometry::{CELL, FILLET_INSET, FILLET_STEPS, cell_world, fillet_polyline, point_world};
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

/// A train's drawable body polyline, head-first: `segs` is `(head-near point,
/// tail-near point, segment length LE)`; `max_d` caps drawing at the real tail.
/// The strand is padded past the tail (fillet math + lag slide room), but a
/// slice must never spill onto that pad — otherwise a still-emerging (just-
/// spawned) train draws its full length onto the fake extension instead of
/// growing in.
struct Body<'a> {
    segs: &'a [(Vec2, Vec2, f32)],
    max_d: f32,
}

/// Draws the body sub-interval `[d0, d1]` (LE from the head) as one or more
/// bands, clipping the interval per segment (so a slice may span several track
/// edges/curves) and capping at [`Body::max_d`].
fn body_slice(commands: &mut Commands, body: &Body, d0: f32, d1: f32, width: f32, color: Color, z: f32) {
    let d1 = d1.min(body.max_d);
    let mut cursor = 0.0;
    for &(head_pt, tail_pt, len) in body.segs {
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

/// World point at arc-length `s` (LE) along the head-first body polyline,
/// clamped to the strand's ends. Lets the head marker / label ride the same
/// on-path interpolated position the body is drawn from.
fn point_at(segs: &[(Vec2, Vec2, f32)], s: f32) -> Vec2 {
    let mut cursor = 0.0;
    for &(a, b, len) in segs {
        if len <= 0.0 {
            continue;
        }
        if s <= cursor + len {
            return a.lerp(b, ((s - cursor) / len).clamp(0.0, 1.0));
        }
        cursor += len;
    }
    segs.last().map_or(Vec2::ZERO, |&(_, b, _)| b)
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

    // Bands, tagged with their block for live recolouring. A plain piece bend
    // (a non-switch Centre node with exactly two legs) is filleted: both legs
    // are pulled back to tangent points and the corner bridged by short chords,
    // so the rail — and the train riding it — sweeps the turn instead of
    // snapping around a hard elbow. Switch/crossing centres (≠2 legs) and
    // straight pass-throughs keep their full-length legs. Every stub and chord
    // still carries `BlockBand`, so live recolour is unaffected.
    let spawn_band = |commands: &mut Commands, a: Vec2, b: Vec2, block: BlockId| {
        let (color, width) = band_style(block, &occupied, &reserved);
        let entity = band(commands, a, b, width, color, 2.0, Tag::Live);
        commands.entity(entity).insert(BlockBand {
            block,
            reserved_was: reserved.contains(&block),
            pulse: 0.0,
        });
    };

    /// A plain-piece bend to round: its centre `c` and the two tangent points
    /// (`ta` on `a_node`'s leg, `tb` on `b_node`'s leg), with the shared block.
    struct Corner {
        c: Vec2,
        ta: Vec2,
        tb: Vec2,
        a_node: u32,
        b_node: u32,
        block: BlockId,
    }
    // Centre node -> its incident legs as (connector node, canonical edge).
    let mut center_legs: BTreeMap<u32, Vec<(u32, u32)>> = BTreeMap::new();
    for (i, edge) in graph.edges.iter().enumerate() {
        if matches!(graph.node(edge.from).kind, NodeKind::Center { .. }) {
            let canon = (i as u32).min(edge.opposite.0);
            center_legs.entry(edge.from.0).or_default().push((edge.to.0, canon));
        }
    }
    let mut corners: BTreeMap<u32, Corner> = BTreeMap::new();
    for (&center, legs) in &center_legs {
        if legs.len() != 2 {
            continue; // switch, flat crossing or stub — leave the elbow alone
        }
        if matches!(graph.node(NodeId(center)).kind, NodeKind::Center { switch: Some(_) }) {
            continue;
        }
        let c = point_world(graph.node(NodeId(center)).point);
        let pa = point_world(graph.node(NodeId(legs[0].0)).point);
        let pb = point_world(graph.node(NodeId(legs[1].0)).point);
        let (Some(da), Some(db)) = ((pa - c).try_normalize(), (pb - c).try_normalize()) else {
            continue;
        };
        if da.dot(db) <= -0.99 {
            continue; // straight pass-through — nothing to round
        }
        // One symmetric pullback for both legs (matches `fillet_polyline`, so
        // the train body stays concentric with the rail through the bend).
        let d = FILLET_INSET.min(c.distance(pa) * 0.5).min(c.distance(pb) * 0.5);
        corners.insert(
            center,
            Corner {
                c,
                ta: c + da * d,
                tb: c + db * d,
                a_node: legs[0].0,
                b_node: legs[1].0,
                block: graph.blocks.block_of(EdgeId(legs[0].1)),
            },
        );
    }
    // Legs: connector → centre, shortened to the tangent point at a filleted bend.
    for (i, edge) in graph.edges.iter().enumerate() {
        if edge.opposite.0 < i as u32 {
            continue; // canonical direction only
        }
        let block = graph.blocks.block_of(EdgeId(i as u32));
        let (center_id, conn_id) = if matches!(graph.node(edge.from).kind, NodeKind::Center { .. }) {
            (edge.from.0, edge.to.0)
        } else {
            (edge.to.0, edge.from.0)
        };
        let conn_px = point_world(graph.node(NodeId(conn_id)).point);
        let end = match corners.get(&center_id) {
            Some(cor) if conn_id == cor.a_node => cor.ta,
            Some(cor) if conn_id == cor.b_node => cor.tb,
            _ => point_world(graph.node(NodeId(center_id)).point),
        };
        spawn_band(&mut commands, conn_px, end, block);
    }
    // Corner arcs: a Bézier ta → centre(control) → tb, tessellated into chords.
    for cor in corners.values() {
        let mut prev = cor.ta;
        for s in 1..=FILLET_STEPS {
            let t = s as f32 / FILLET_STEPS as f32;
            let mt = 1.0 - t;
            let p = cor.ta * (mt * mt) + cor.c * (2.0 * mt * t) + cor.tb * (t * t);
            spawn_band(&mut commands, prev, p, cor.block);
            prev = p;
        }
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
    // Parallel to `segs`: is each segment's tail node a rail-filleted bend (a
    // 2-leg, non-switch centre)? Drives the body fillet mask so the train rounds
    // exactly the corners the rail rounds and stays on the track through a bend.
    let mut tail_bend: Vec<bool> = Vec::new();
    // On-path interpolated head per train, shared with the label pass below so
    // the number rides the same smoothed position as the body.
    let mut heads_on_path: BTreeMap<TrainId, Vec2> = BTreeMap::new();
    for train in sim.trains() {
        let class = train.class;
        let loco_col = col_class_loco(class);
        // Body polyline head-first: `hi` is the head-near end of each edge (the
        // train travels from→to, head toward `to`).
        train.occupied_into(graph, &mut buf);
        segs.clear();
        tail_bend.clear();
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
            // The tail-side node (`from`, head travels toward `to`) is the shared
            // vertex with the next edge back. Round it only where the rail does:
            // a non-switch centre. Connector and switch elbows stay sharp.
            tail_bend.push(matches!(graph.node(data.from).kind, NodeKind::Center { switch: None }));
        }
        // Interpolate the head ALONG the path, not by a straight world-space
        // lerp. The sim advances in 10 Hz steps; between ticks the train is drawn
        // a sub-tick `lag` behind its current-tick head. The old code slid the
        // whole strand by that world vector — on a bend the offset pointed across
        // the corner chord and its direction swung each tick, which is exactly
        // the curve jitter. Instead we keep the body on its own polyline and just
        // start drawing `off` LE into it, so the train always rides the rail.
        // LE↔px is uniform across the frozen unit table (HALF_DIAGONAL =
        // round(HALF_CARDINAL·√2)), so one representative segment fixes the scale.
        let Some(px_per_le) = segs
            .iter()
            .find(|s| s.2 > 0.0)
            .map(|s| s.0.distance(s.1) / s.2)
            .filter(|k| *k > 0.0)
        else {
            continue; // nothing drawable this frame
        };
        let lag_px = ctl.head_lag_px(train.id);
        // Real emerged length (before padding) — the drawn body is capped here so
        // a just-spawned train grows in from the source instead of popping its
        // full length onto the tail pad.
        let occupied_le: f32 = segs.iter().map(|s| s.2).sum();
        // Round the body's corners so a train sweeps a bend concentric with the
        // filleted rail. Pad BOTH ends to a full leg first: the occupied span
        // ends mid-edge, and a short end-leg would clamp the fillet radius tighter
        // than the rail's full-leg fillet — the body would then bulge OUTSIDE the
        // rail on the bend nearest the head/tail. The pad is collinear with the
        // end edge (drawn nowhere, feeds only the corner math); the real body is
        // drawn from `off`. This also gives the tail room to slide for the lag.
        // LE↔px is uniform across the frozen unit table, so one segment fixes it.
        const PAD: f32 = 2.0 * FILLET_INSET + 6.0;
        let head_dir = segs.first().and_then(|&(a, b, _)| (a - b).try_normalize());
        let tail_dir = segs.last().and_then(|&(a, b, _)| (b - a).try_normalize());
        let mut pts = Vec::with_capacity(segs.len() + 3);
        // `mask` runs parallel to `pts`: only rail-filleted centre vertices are
        // rounded (see `tail_bend`). The pads and the head point are collinear
        // fillers, never real bends, so they are masked off.
        let mut mask = Vec::with_capacity(segs.len() + 3);
        if let Some(hd) = head_dir {
            pts.push(segs[0].0 + hd * PAD);
            mask.push(false);
        }
        pts.push(segs[0].0);
        mask.push(false);
        for (s, &bend) in segs.iter().zip(&tail_bend) {
            pts.push(s.1);
            mask.push(bend);
        }
        if let (Some(td), Some(&(_, last, _))) = (tail_dir, segs.last()) {
            pts.push(last + td * PAD);
            mask.push(false);
        }
        let rounded = fillet_polyline(&pts, Some(&mask), FILLET_INSET, FILLET_STEPS);
        segs.clear();
        for w in rounded.windows(2) {
            segs.push((w[0], w[1], w[0].distance(w[1]) / px_per_le));
        }
        // Draw from the real head: skip the leading pad, then the sub-tick lag.
        let head_pad_le = head_dir.map_or(0.0, |_| PAD) / px_per_le;
        let off = head_pad_le + lag_px / px_per_le;
        // Cap every slice at the tail. A fully-on-board train may spill the last
        // `lag` onto the tail pad (its interpolated tail sits there — the slide
        // that keeps the tail gliding, not popping, each tick). A train still
        // growing in has its tail pinned at the source, so we clamp at the REAL
        // tail instead — otherwise the body slides onto the pad *past* the source
        // (off the board, below "Start") and the emergence pops. Trains vanish
        // whole at a sink (no gradual exit), so `occupied < length` ⇒ spawning.
        let max_d = if occupied_le + 0.5 >= train.length.0 as f32 {
            off + occupied_le
        } else {
            head_pad_le + occupied_le
        };
        let body = Body { segs: &segs, max_d };
        // Loco (bright front), then each wagon behind its darker coupling. A slice
        // past the tail draws nothing, so a partial last wagon clips cleanly.
        body_slice(&mut commands, &body, off, off + LOCO_LEN, LOCO_W, loco_col, 10.1);
        let wagon_col = col_class_wagon(class);
        let wagons = (((train.length.0 - LOCO_LEN as i64).max(0)) as f32 / SLOT).round() as usize;
        for i in 0..wagons {
            let base = off + LOCO_LEN + i as f32 * SLOT;
            body_slice(&mut commands, &body, base, base + COUPLING_LEN, COUPLING_W, col_coupling(), 10.0);
            body_slice(&mut commands, &body, base + COUPLING_LEN, base + SLOT, WAGON_W, wagon_col, 10.0);
            // Freight (class 1): inset grey cargo → open gondola look.
            if class.0 == 1 {
                body_slice(
                    &mut commands,
                    &body,
                    base + COUPLING_LEN + CARGO_INSET,
                    base + SLOT - CARGO_INSET,
                    CARGO_W,
                    col_cargo(),
                    10.3,
                );
            }
        }
        // Loco head marker (class silhouette) at the on-path interpolated head —
        // the second, colour-blind-safe class cue. 0 square · 1 diamond · 2 chevron.
        // Oriented to the LOCAL travel direction (the body's tangent at the head),
        // so the marker follows the rail through a bend instead of sitting square
        // to the screen. Falls back to the head-edge direction if the tangent is
        // degenerate (e.g. a just-spawned zero-length body).
        let head = point_at(&segs, off);
        heads_on_path.insert(train.id, head);
        let heading = {
            let tangent = (head - point_at(&segs, off + 24.0)).normalize_or_zero();
            if tangent == Vec2::ZERO {
                let d = graph.edge(train.head_edge());
                (point_world(graph.node(d.to).point) - point_world(graph.node(d.from).point))
                    .normalize_or_zero()
            } else {
                tangent
            }
        };
        let angle = heading.to_angle();
        match class.0 {
            2 => {
                let perp = heading.perp();
                let back = head - heading * 9.0;
                for e in [
                    band(&mut commands, head, back + perp * 7.2, 3.0, loco_col, 11.0, Tag::Live),
                    band(&mut commands, head, back - perp * 7.2, 3.0, loco_col, 11.0, Tag::Live),
                ] {
                    commands.entity(e).insert(TrainGfx);
                }
            }
            c => {
                // Square (0) or diamond (1): a diamond is the square turned 45°.
                let spin = angle + if c == 1 { std::f32::consts::FRAC_PI_4 } else { 0.0 };
                let mut e = commands.spawn((
                    Sprite::from_color(loco_col, Vec2::splat(13.0)),
                    Transform::from_translation(head.extend(11.0))
                        .with_rotation(Quat::from_rotation_z(spin)),
                ));
                Tag::Live.apply(&mut e);
                e.insert(TrainGfx);
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
        let head = heads_on_path
            .get(&train.id)
            .copied()
            .unwrap_or_else(|| ctl.interpolated_head(train.id));
        let pos = head + off;
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
