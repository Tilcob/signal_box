//! Drawing primitives (bands, lamps, labels) and the layout/station
//! renderers shared by the edit board and the run board.

use bevy::prelude::*;
use bevy::text::Font;
use std::collections::BTreeMap;
use stellwerk_sim::grid::{Cell, Dir8};
use stellwerk_sim::layout::{Layout, SignalKind};
use stellwerk_sim::level::Level;
use stellwerk_sim::units::BlockId;

/// Per-stub block colours for the edit board (`(cell, connector) → block`).
/// Built best-effort from the live graph; `None` when the layout does not yet
/// validate, in which case bands fall back to their flat colour.
pub(super) type BlockColors = BTreeMap<(Cell, Dir8), BlockId>;

use super::geometry::{CELL, blocked_cells, cell_world, connector_world};
use super::palette::*;
use crate::i18n::{dir_label, sink_label, source_label};

/// Static board sprites (rebuilt on layout change / mode entry).
#[derive(Component)]
pub(super) struct BoardGfx;

/// Per-frame run sprites (trains, lit bands, lamps).
#[derive(Component)]
pub(super) struct LiveGfx;

/// Which marker the spawned entity gets.
#[derive(Clone, Copy)]
pub(super) enum Tag {
    Board,
    Live,
}

impl Tag {
    pub(super) fn apply(self, entity: &mut EntityCommands) {
        match self {
            Tag::Board => entity.insert(BoardGfx),
            Tag::Live => entity.insert(LiveGfx),
        };
    }
}

pub(super) fn despawn_all<C: Component>(mut commands: Commands, q: Query<Entity, With<C>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

/// Spawns a coloured band and returns its entity, so run-board callers can
/// attach state markers (`BlockBand`) for per-frame in-place recolouring.
pub(super) fn band(
    commands: &mut Commands,
    a: Vec2,
    b: Vec2,
    width: f32,
    color: Color,
    z: f32,
    tag: Tag,
) -> Entity {
    let mid = (a + b) / 2.0;
    let delta = b - a;
    let mut entity = commands.spawn((
        Sprite::from_color(color, Vec2::new(delta.length().max(1.0), width)),
        Transform::from_translation(mid.extend(z))
            .with_rotation(Quat::from_rotation_z(delta.y.atan2(delta.x))),
    ));
    tag.apply(&mut entity);
    entity.id()
}

pub(super) fn lamp(
    commands: &mut Commands,
    pos: Vec2,
    size: f32,
    color: Color,
    diamond: bool,
    z: f32,
    tag: Tag,
) -> Entity {
    let rot = if diamond {
        Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)
    } else {
        Quat::IDENTITY
    };
    let mut entity = commands.spawn((
        Sprite::from_color(color, Vec2::splat(size)),
        Transform::from_translation(pos.extend(z)).with_rotation(rot),
    ));
    tag.apply(&mut entity);
    entity.id()
}

pub(super) fn label(
    commands: &mut Commands,
    font: &Handle<Font>,
    pos: Vec2,
    text: String,
    size: f32,
    color: Color,
    tag: Tag,
) -> Entity {
    let mut entity = commands.spawn((
        Text2d::new(text),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
        Transform::from_translation(pos.extend(6.0)),
    ));
    tag.apply(&mut entity);
    entity.id()
}

/// Signal lamp position: at the connector, offset perpendicular to the stub.
pub(super) fn signal_pos(cell: Cell, at: Dir8) -> Vec2 {
    let connector = connector_world(cell, at);
    let inward = (cell_world(cell) - connector).normalize_or_zero();
    connector + inward.perp() * 16.0
}

/// Arrow drawn ON the gated stub, pointing in the one travel direction the
/// signal guards (trains leaving the cell across `at`). Replaces the easy-to-
/// miss tick: a signal placed backwards now visibly points the wrong way —
/// the classic "my signal does nothing" trap is now legible on the track.
/// Returns the sprite entities (shaft + two wings) so the run board can mark
/// them all for per-frame recolour.
pub(super) fn signal_arrow(
    commands: &mut Commands,
    cell: Cell,
    at: Dir8,
    color: Color,
    tag: Tag,
) -> Vec<Entity> {
    let center = cell_world(cell);
    let connector = connector_world(cell, at);
    let outward = (connector - center).normalize_or_zero();
    let perp = outward.perp();
    let tip = center.lerp(connector, 0.72);
    let tail = center.lerp(connector, 0.38);
    let back = tip - outward * 9.0;
    vec![
        band(commands, tail, tip, 3.0, color, 5.0, tag),
        band(commands, tip, back + perp * 7.0, 3.0, color, 5.0, tag),
        band(commands, tip, back - perp * 7.0, 3.0, color, 5.0, tag),
    ]
}

pub(super) fn draw_layout(
    commands: &mut Commands,
    font: &Handle<Font>,
    layout: &Layout,
    color: Color,
    blocks: Option<&BlockColors>,
    tag: Tag,
) {
    // A stub's colour: its block hue when the partition is known, else the
    // flat layout colour (fixed/player) passed in.
    let band_color = |cell: Cell, dir: Dir8| {
        blocks
            .and_then(|m| m.get(&(cell, dir)))
            .map_or(color, |&b| col_block(b))
    };
    for piece in &layout.pieces {
        let center = cell_world(piece.cell);
        band(
            commands,
            connector_world(piece.cell, piece.a),
            center,
            7.0,
            band_color(piece.cell, piece.a),
            2.0,
            tag,
        );
        band(
            commands,
            connector_world(piece.cell, piece.b),
            center,
            7.0,
            band_color(piece.cell, piece.b),
            2.0,
            tag,
        );
    }
    for switch in &layout.switches {
        let center = cell_world(switch.cell);
        band(
            commands,
            connector_world(switch.cell, switch.stem),
            center,
            7.0,
            band_color(switch.cell, switch.stem),
            2.0,
            tag,
        );
        for (i, branch) in switch.branches.iter().enumerate() {
            let is_default = i as u8 == switch.default_branch;
            let branch_color = if is_default {
                col_switch_active()
            } else {
                col_switch_inactive()
            };
            band(
                commands,
                connector_world(switch.cell, *branch),
                center,
                7.0,
                branch_color,
                3.0,
                tag,
            );
            // Compass label at the exit — the same name the config panel
            // uses, so "Ziel OST → O" is locatable on the board. Offset
            // perpendicular to the stub, otherwise it sits ON the track.
            let connector = connector_world(switch.cell, *branch);
            let outward = (connector - center).normalize_or_zero();
            label(
                commands,
                font,
                connector + outward.perp() * 16.0,
                dir_label(*branch),
                11.0,
                if is_default {
                    col_switch_active()
                } else {
                    col_label()
                },
                tag,
            );
        }
        lamp(commands, center, 12.0, col_switch_active(), true, 4.0, tag);
        if !switch.rules.is_empty() {
            label(
                commands,
                font,
                center + Vec2::new(0.0, 18.0),
                format!("{}R", switch.rules.len()),
                12.0,
                col_label(),
                tag,
            );
        }
    }
    for signal in &layout.signals {
        lamp(
            commands,
            signal_pos(signal.cell, signal.at),
            14.0,
            col_signal_green(),
            matches!(signal.kind, SignalKind::Chain),
            5.0,
            tag,
        );
        signal_arrow(commands, signal.cell, signal.at, col_signal_green(), tag);
    }
}

/// Solid tiles for the sandbox's blocked (non-buildable) holes, derived from
/// `buildable` via [`super::geometry::blocked_cells`]. Same z as the grid
/// (0.0); a blocked cell has no grid tile, so the two never overlap.
pub(super) fn draw_blocks(commands: &mut Commands, buildable: &[Cell], tag: Tag) {
    for cell in blocked_cells(buildable) {
        let mut entity = commands.spawn((
            Sprite::from_color(col_blocked(), Vec2::splat(CELL - 4.0)),
            Transform::from_translation(cell_world(cell).extend(0.0)),
        ));
        tag.apply(&mut entity);
    }
}

/// A two-winged chevron meeting at `tip`, pointing along the unit vector `dir`.
/// Built from the same bands as the signal arrow's head; used to mark a
/// source's travel direction (trains enter the cell this way).
fn chevron(commands: &mut Commands, tip: Vec2, dir: Vec2, size: f32, color: Color, tag: Tag) {
    let perp = dir.perp();
    let back = tip - dir * size;
    band(commands, tip, back + perp * size * 0.8, 3.0, color, 2.5, tag);
    band(commands, tip, back - perp * size * 0.8, 3.0, color, 2.5, tag);
}

/// Stations get distinct silhouettes so neither reads as a signal (a lamp on
/// the stub) or plain track: a source is an outward stub with chevrons marching
/// INTO the cell ("trains enter here"); a sink is an outward stub capped by a
/// buffer-stop bar ("end of the line"). Each in its own palette colour.
pub(super) fn draw_stations(commands: &mut Commands, font: &Handle<Font>, level: &Level, tag: Tag) {
    for source in &level.sources {
        let connector = connector_world(source.cell, source.dir);
        let outward = (connector - cell_world(source.cell)).normalize_or_zero();
        band(
            commands,
            connector,
            connector + outward * 26.0,
            8.0,
            col_source(),
            2.0,
            tag,
        );
        // Two chevrons pointing inward (into the cell): the way trains arrive.
        for i in 0..2 {
            let tip = connector - outward * (2.0 + i as f32 * 10.0);
            chevron(commands, tip, -outward, 9.0, col_source(), tag);
        }
        label(
            commands,
            font,
            connector + outward * 58.0,
            source_label(source.id.0, &source.label),
            13.0,
            col_label(),
            tag,
        );
    }
    for sink in &level.sinks {
        let connector = connector_world(sink.cell, sink.dir);
        let outward = (connector - cell_world(sink.cell)).normalize_or_zero();
        let end = connector + outward * 22.0;
        band(commands, connector, end, 8.0, col_sink(), 2.0, tag);
        // Buffer-stop bar across the stub end: the track terminates here.
        let perp = outward.perp();
        band(commands, end - perp * 13.0, end + perp * 13.0, 6.0, col_sink(), 2.5, tag);
        label(
            commands,
            font,
            connector + outward * 26.0 + Vec2::new(0.0, 26.0),
            sink_label(sink.id.0, &sink.label),
            14.0,
            col_label(),
            tag,
        );
    }
}
