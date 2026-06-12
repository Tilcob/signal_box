//! Mouse input: click a signal lamp to toggle red/green, click a switch node
//! to flip the branch. Works while paused too — pause-and-plan is part of the
//! genre.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::core::{GameState, Tunables};
use crate::render::MainCamera;
use crate::sim::{Signal, TrackGraph};

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            handle_clicks.run_if(in_state(GameState::Playing)),
        );
    }
}

fn cursor_world_pos(
    window: &Window,
    camera: &Camera,
    camera_transform: &GlobalTransform,
) -> Option<Vec2> {
    let cursor = window.cursor_position()?;
    camera.viewport_to_world_2d(camera_transform, cursor).ok()
}

fn handle_clicks(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    tunables: Res<Tunables>,
    mut graph: ResMut<TrackGraph>,
    mut signals: Query<(&mut Signal, &Transform)>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let (Ok(window), Ok((camera, camera_transform))) = (windows.single(), cameras.single())
    else {
        return;
    };
    let Some(click) = cursor_world_pos(window, camera, camera_transform) else {
        return;
    };

    // Signals first (their lamps sit right next to the track), then switches.
    let mut best_signal: Option<(f32, Mut<Signal>)> = None;
    for (signal, transform) in &mut signals {
        let d = transform.translation.truncate().distance(click);
        if d <= tunables.click_radius
            && best_signal.as_ref().is_none_or(|(bd, _)| d < *bd)
        {
            best_signal = Some((d, signal));
        }
    }
    if let Some((_, mut signal)) = best_signal {
        signal.green = !signal.green;
        return;
    }

    let click_radius = tunables.click_radius;
    let switch_index = graph
        .switches
        .iter()
        .enumerate()
        .filter(|(_, s)| graph.pos(s.node).distance(click) <= click_radius)
        .min_by(|(_, s1), (_, s2)| {
            let d1 = graph.pos(s1.node).distance(click);
            let d2 = graph.pos(s2.node).distance(click);
            d1.total_cmp(&d2)
        })
        .map(|(i, _)| i);
    if let Some(i) = switch_index {
        let switch = &mut graph.switches[i];
        switch.selected = 1 - switch.selected;
    }
}
