//! 2D camera, pan and zoom. The Pult glow (HDR + bloom) is opt-in via
//! `STELLWERK_BLOOM` — off by default because the full-screen post-process
//! is the dominant frame cost at high resolutions (see `spawn_camera`).
//!
//! Note: plain Bevy input for now; the `bevy_enhanced_input` action maps
//! come with the rebinding UI.

use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::state::FocusedField;

#[derive(Component)]
pub struct MainCamera;

/// Zoom factor: world is shown at `zoom`× magnification.
#[derive(Resource)]
pub struct Zoom(pub f32);

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Zoom(1.0))
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, (pan, zoom));
    }
}

fn spawn_camera(mut commands: Commands) {
    let mut camera = commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection::default_2d()),
        Transform::from_xyz(0.0, 0.0, 999.0),
        MainCamera,
        Name::new("MainCamera"),
    ));
    // The Pult glow is HDR + a full-screen bloom post-process. Its cost scales
    // with the window resolution (≈2× frame time already at 900p, far worse at
    // fullscreen 1440p/4K, where it is the dominant frame cost). Off by default
    // so the game is playable everywhere; opt in with STELLWERK_BLOOM=1 on a
    // GPU that can afford it.
    if std::env::var_os("STELLWERK_BLOOM").is_some() {
        camera.insert((bevy::render::view::Hdr, Bloom::default()));
    }
}

fn pan(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<FocusedField>,
    mut motion: MessageReader<MouseMotion>,
    time: Res<Time>,
    zoom: Res<Zoom>,
    mut cameras: Query<&mut Transform, With<MainCamera>>,
) {
    // Drag with middle/right mouse button… (always drain the motion events,
    // even when not dragging, so they don't pile up).
    let dragging = buttons.pressed(MouseButton::Middle) || buttons.pressed(MouseButton::Right);
    let mut delta = Vec2::ZERO;
    for event in motion.read() {
        if dragging {
            delta += event.delta;
        }
    }
    // …or WASD/arrows — but NOT while a text/number field is focused, or typing
    // a station name (or just being in a field) would scroll the board.
    let mut dir = Vec2::ZERO;
    if focus.0.is_none() {
        if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
            dir.y += 1.0;
        }
        if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
            dir.y -= 1.0;
        }
        if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
            dir.x -= 1.0;
        }
        if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
            dir.x += 1.0;
        }
    }

    // No input → do NOT touch the Transform. Writing it (even by +0.0) trips
    // change detection every frame, forcing GlobalTransform propagation and a
    // 2D visibility recompute over every board sprite — in every state.
    if delta == Vec2::ZERO && dir == Vec2::ZERO {
        return;
    }
    let Ok(mut transform) = cameras.single_mut() else {
        return;
    };
    transform.translation.x -= delta.x / zoom.0;
    transform.translation.y += delta.y / zoom.0;
    let speed = 600.0 / zoom.0;
    transform.translation.x += dir.x * speed * time.delta_secs();
    transform.translation.y += dir.y * speed * time.delta_secs();
}

fn zoom(
    mut wheel: MessageReader<MouseWheel>,
    hovered: Res<crate::console::ConsoleHovered>,
    focus: Res<FocusedField>,
    mut zoom: ResMut<Zoom>,
    mut cameras: Query<&mut Projection, With<MainCamera>>,
) {
    // The console scrolls on the same wheel events; while the pointer is over it
    // the board must not zoom. Consume the events so they don't backlog into a
    // late zoom once the pointer leaves the console.
    if hovered.0 {
        wheel.read().count();
        return;
    }
    let mut steps = 0.0;
    for event in wheel.read() {
        // Native reports Line units (±1 per notch); browsers report Pixel units
        // (~100 per notch), which would send `1.15^steps` straight to the clamp —
        // i.e. one notch zooming the whole range. Normalize pixels back to notches.
        steps += match event.unit {
            MouseScrollUnit::Line => event.y,
            MouseScrollUnit::Pixel => event.y / 100.0,
        };
    }
    // Drained above; skip zooming while a field is focused (same input-leak
    // class as the WASD pan).
    if steps == 0.0 || focus.0.is_some() {
        return;
    }
    zoom.0 = (zoom.0 * 1.15f32.powf(steps)).clamp(0.25, 4.0);
    if let Ok(mut projection) = cameras.single_mut()
        && let Projection::Orthographic(ortho) = &mut *projection
    {
        ortho.scale = 1.0 / zoom.0;
    }
}

/// Cursor position in world coordinates, if over the window.
pub fn cursor_world(
    windows: &Query<&Window, With<PrimaryWindow>>,
    cameras: &Query<(&Camera, &GlobalTransform), With<MainCamera>>,
) -> Option<Vec2> {
    let window = windows.single().ok()?;
    let (camera, transform) = cameras.single().ok()?;
    let cursor = window.cursor_position()?;
    camera.viewport_to_world_2d(transform, cursor).ok()
}
