//! Fixed camera, station decorations (sprites + labels), and per-frame gizmo
//! drawing of tracks and switches. Prototype look: clean lines, no assets.

use bevy::prelude::*;

use crate::sim::{NodeKind, TrackGraph};

const CAMERA_Z: f32 = 999.0;
const STATION_Z: f32 = 2.0;
/// Half-gap between the two rails of a track.
const RAIL_OFFSET: f32 = 2.5;
/// Length of the switch direction indicator.
const SWITCH_ARM: f32 = 30.0;

#[derive(Component)]
pub struct MainCamera;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_camera, spawn_stations))
            .add_systems(Update, (draw_tracks, draw_switches));
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Transform::from_xyz(0.0, 0.0, CAMERA_Z),
        MainCamera,
        Name::new("MainCamera"),
    ));
}

fn spawn_stations(mut commands: Commands, graph: Res<TrackGraph>) {
    for node in &graph.nodes {
        match node.kind {
            NodeKind::Source(label) => {
                commands.spawn((
                    Sprite::from_color(Color::srgb(0.45, 0.45, 0.50), Vec2::new(26.0, 34.0)),
                    Transform::from_translation(node.pos.extend(STATION_Z)),
                    Name::new(format!("Source {label}")),
                ));
                commands.spawn((
                    Text2d::new(label),
                    TextFont::from_font_size(18.0),
                    TextColor(Color::srgb(0.85, 0.85, 0.9)),
                    Transform::from_translation((node.pos + Vec2::new(0.0, 34.0)).extend(STATION_Z)),
                ));
            }
            NodeKind::Sink(cargo) => {
                commands.spawn((
                    Sprite::from_color(cargo.color(), Vec2::new(34.0, 34.0)),
                    Transform::from_translation(node.pos.extend(STATION_Z)),
                    Name::new(format!("Sink {}", cargo.name())),
                ));
                commands.spawn((
                    Text2d::new(cargo.name()),
                    TextFont::from_font_size(14.0),
                    TextColor(cargo.color()),
                    Transform::from_translation((node.pos + Vec2::new(0.0, 34.0)).extend(STATION_Z)),
                ));
            }
            NodeKind::Plain => {}
        }
    }
}

/// Each edge as two parallel rails.
fn draw_tracks(mut gizmos: Gizmos, graph: Res<TrackGraph>) {
    let rail_color = Color::srgb(0.40, 0.42, 0.46);
    for edge in &graph.edges {
        let (a, b) = (graph.pos(edge.a), graph.pos(edge.b));
        let offset = (b - a).normalize().perp() * RAIL_OFFSET;
        gizmos.line_2d(a + offset, b + offset, rail_color);
        gizmos.line_2d(a - offset, b - offset, rail_color);
    }
}

/// Switch node: a circle plus a bright arm pointing toward the selected
/// branch and a dim arm toward the other one.
fn draw_switches(mut gizmos: Gizmos, graph: Res<TrackGraph>) {
    for switch in &graph.switches {
        let center = graph.pos(switch.node);
        gizmos.circle_2d(
            Isometry2d::from_translation(center),
            12.0,
            Color::srgb(0.85, 0.75, 0.20),
        );
        for (i, &option) in switch.options.iter().enumerate() {
            let dir = (graph.pos(option) - center).normalize();
            let color = if i == switch.selected {
                Color::srgb(0.95, 0.85, 0.25)
            } else {
                Color::srgb(0.30, 0.28, 0.18)
            };
            gizmos.line_2d(center, center + dir * SWITCH_ARM, color);
        }
    }
}
