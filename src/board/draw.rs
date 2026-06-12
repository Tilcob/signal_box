//! Drawing primitives (bands, lamps, labels) and the layout/station
//! renderers shared by the edit board and the run board.

use bevy::prelude::*;
use bevy::text::Font;
use stellwerk_sim::grid::{Cell, Dir8};
use stellwerk_sim::layout::{Layout, SignalKind};
use stellwerk_sim::level::Level;

use super::geometry::{cell_world, connector_world};
use super::palette::*;
use crate::i18n::dir_label;

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
    pub(super) fn apply(self, mut entity: bevy::ecs::system::EntityCommands) {
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

pub(super) fn band(
    commands: &mut Commands,
    a: Vec2,
    b: Vec2,
    width: f32,
    color: Color,
    z: f32,
    tag: Tag,
) {
    let mid = (a + b) / 2.0;
    let delta = b - a;
    let entity = commands.spawn((
        Sprite::from_color(color, Vec2::new(delta.length().max(1.0), width)),
        Transform::from_translation(mid.extend(z))
            .with_rotation(Quat::from_rotation_z(delta.y.atan2(delta.x))),
    ));
    tag.apply(entity);
}

pub(super) fn lamp(
    commands: &mut Commands,
    pos: Vec2,
    size: f32,
    color: Color,
    diamond: bool,
    z: f32,
    tag: Tag,
) {
    let rot = if diamond {
        Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)
    } else {
        Quat::IDENTITY
    };
    let entity = commands.spawn((
        Sprite::from_color(color, Vec2::splat(size)),
        Transform::from_translation(pos.extend(z)).with_rotation(rot),
    ));
    tag.apply(entity);
}

pub(super) fn label(
    commands: &mut Commands,
    font: &Handle<Font>,
    pos: Vec2,
    text: String,
    size: f32,
    color: Color,
    tag: Tag,
) {
    let entity = commands.spawn((
        Text2d::new(text),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
        Transform::from_translation(pos.extend(6.0)),
    ));
    tag.apply(entity);
}

/// Signal lamp position: at the connector, offset perpendicular to the stub.
pub(super) fn signal_pos(cell: Cell, at: Dir8) -> Vec2 {
    let connector = connector_world(cell, at);
    let inward = (cell_world(cell) - connector).normalize_or_zero();
    connector + inward.perp() * 16.0
}

/// Direction tick next to the lamp: a signal gates exactly ONE travel
/// direction (trains leaving the cell across its connector). Without the
/// tick a signal placed backwards looks identical to a working one — the
/// classic "my signal does nothing" trap.
pub(super) fn signal_direction_tick(
    commands: &mut Commands,
    cell: Cell,
    at: Dir8,
    color: Color,
    tag: Tag,
) {
    let connector = connector_world(cell, at);
    let outward = (connector - cell_world(cell)).normalize_or_zero();
    let base = signal_pos(cell, at);
    band(commands, base, base + outward * 16.0, 3.0, color, 5.0, tag);
}

pub(super) fn draw_layout(
    commands: &mut Commands,
    font: &Handle<Font>,
    layout: &Layout,
    color: Color,
    tag: Tag,
) {
    for piece in &layout.pieces {
        let center = cell_world(piece.cell);
        band(
            commands,
            connector_world(piece.cell, piece.a),
            center,
            7.0,
            color,
            2.0,
            tag,
        );
        band(
            commands,
            connector_world(piece.cell, piece.b),
            center,
            7.0,
            color,
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
            color,
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
        signal_direction_tick(commands, signal.cell, signal.at, col_signal_green(), tag);
    }
}

pub(super) fn draw_stations(commands: &mut Commands, font: &Handle<Font>, level: &Level, tag: Tag) {
    for source in &level.sources {
        let connector = connector_world(source.cell, source.dir);
        let outward = (connector - cell_world(source.cell)).normalize_or_zero();
        band(
            commands,
            connector,
            connector + outward * 30.0,
            10.0,
            col_fixed(),
            2.0,
            tag,
        );
        label(
            commands,
            font,
            connector + outward * 58.0,
            format!("Q{}", source.id.0),
            13.0,
            col_label(),
            tag,
        );
    }
    for sink in &level.sinks {
        let connector = connector_world(sink.cell, sink.dir);
        let outward = (connector - cell_world(sink.cell)).normalize_or_zero();
        band(
            commands,
            connector,
            connector + outward * 30.0,
            10.0,
            col_fixed(),
            2.0,
            tag,
        );
        label(
            commands,
            font,
            connector + outward * 26.0 + Vec2::new(0.0, 26.0),
            sink.label.clone(),
            14.0,
            col_label(),
            tag,
        );
    }
}
