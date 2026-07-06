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
use stellwerk_sim::Level;
use stellwerk_sim::grid::Cell;

use crate::board::{self, CELL};
use crate::state::{ActiveLevel, FocusedField, TimetableHovered};

/// Zoom clamp. The lower bound is deliberately small so the biggest levels
/// (up to 50×50) fit on screen after the auto-fit.
const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 4.0;

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
            .add_systems(Update, (pan, zoom, fit_camera_to_level));
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
    keys: Res<ButtonInput<KeyCode>>,
    hovered: Res<crate::console::ConsoleHovered>,
    timetable: Res<TimetableHovered>,
    focus: Res<FocusedField>,
    mut zoom: ResMut<Zoom>,
    mut cameras: Query<&mut Projection, With<MainCamera>>,
) {
    // The console AND the scrollable timetable scroll on the same wheel events;
    // while the pointer is over either, the board must not zoom. Ctrl+wheel is
    // reserved for cycling the track curve form (`editor::tools::cycle_track_form`).
    // In every case consume the events so they don't backlog into a late zoom.
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if hovered.0 || timetable.0 || ctrl {
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
    zoom.0 = (zoom.0 * 1.15f32.powf(steps)).clamp(MIN_ZOOM, MAX_ZOOM);
    if let Ok(mut projection) = cameras.single_mut()
        && let Projection::Orthographic(ortho) = &mut *projection
    {
        ortho.scale = 1.0 / zoom.0;
    }
}

/// Cell bounding box over everything a level draws (buildable, fixed track,
/// stations, platforms). `None` for an empty level.
fn level_bounds(level: &Level) -> Option<(Cell, Cell)> {
    let mut cells = level
        .buildable
        .iter()
        .copied()
        .chain(level.fixed.pieces.iter().map(|p| p.cell))
        .chain(level.sources.iter().map(|s| s.cell))
        .chain(level.sinks.iter().map(|s| s.cell))
        .chain(level.platforms.iter().map(|p| p.cell));
    let first = cells.next()?;
    let (mut min, mut max) = (first, first);
    for c in cells {
        min.x = min.x.min(c.x);
        min.y = min.y.min(c.y);
        max.x = max.x.max(c.x);
        max.y = max.y.max(c.y);
    }
    Some((min, max))
}

/// On loading a NEW level, frame its whole extent — big levels otherwise open
/// zoomed into the middle. Keyed on the level id via a `Local`, so it fires
/// once per level and never fights the player's later manual pan/zoom (which
/// also mutate the camera but leave the id unchanged).
fn fit_camera_to_level(
    mut last: Local<Option<String>>,
    active: Option<Res<ActiveLevel>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut zoom: ResMut<Zoom>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<MainCamera>>,
) {
    let Some(active) = active else { return };
    if last.as_deref() == Some(active.id.as_str()) {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let (vw, vh) = (window.width(), window.height());
    if vw < 1.0 || vh < 1.0 {
        return; // window not sized yet — try again next frame (id not stored)
    }
    let Some((min, max)) = level_bounds(&active.level) else {
        return;
    };
    let (lo, hi) = (board::cell_world(min), board::cell_world(max));
    let center = (lo + hi) / 2.0;
    // Span in world px, including the outer cells' own width/height.
    let span = (hi - lo).abs() + Vec2::splat(CELL);
    // Fit into ~82% of the window; the HUD panels hug the edges.
    let fit = (vw * 0.82 / span.x).min(vh * 0.82 / span.y);
    let z = fit.clamp(MIN_ZOOM, MAX_ZOOM);
    zoom.0 = z;
    if let Ok((mut tf, mut projection)) = cameras.single_mut() {
        tf.translation.x = center.x;
        tf.translation.y = center.y;
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scale = 1.0 / z;
        }
    }
    *last = Some(active.id.clone());
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
