//! Block signals: a red signal stops trains travelling the guarded directed
//! edge at the signal's position. Toggled by clicking.

use bevy::prelude::*;

use super::graph::TrackGraph;

const SIGNAL_Z: f32 = 5.0;
/// Signals sit this far before the node their edge leads into.
const DIST_BEFORE_NODE: f32 = 70.0;
/// Perpendicular offset of the signal lamp from the track centerline.
const LAMP_OFFSET: f32 = 18.0;

#[derive(Component)]
pub struct Signal {
    /// The guarded directed edge: applies to trains moving `from` → `to`.
    pub from: usize,
    pub to: usize,
    /// Stop position, measured along the edge from `from`.
    pub dist: f32,
    pub green: bool,
}

/// The two approach signals before the merge — one per source line.
pub fn spawn_signals(mut commands: Commands, graph: Res<TrackGraph>) {
    for (from, to) in [(2usize, 4usize), (3, 4)] {
        let len = graph.edge_len(from, to);
        let dist = len - DIST_BEFORE_NODE;
        let (a, b) = (graph.pos(from), graph.pos(to));
        let dir = (b - a).normalize();
        let lamp_pos = a.lerp(b, dist / len) + dir.perp() * LAMP_OFFSET;

        commands.spawn((
            Signal {
                from,
                to,
                dist,
                green: true,
            },
            Sprite::from_color(Color::WHITE, Vec2::splat(14.0)),
            Transform::from_translation(lamp_pos.extend(SIGNAL_Z)),
            Name::new(format!("Signal {from}->{to}")),
        ));
    }
}

pub fn tint_signal_sprites(mut signals: Query<(&Signal, &mut Sprite), Changed<Signal>>) {
    for (signal, mut sprite) in &mut signals {
        sprite.color = if signal.green {
            Color::srgb(0.20, 0.85, 0.30)
        } else {
            Color::srgb(0.90, 0.18, 0.18)
        };
    }
}
