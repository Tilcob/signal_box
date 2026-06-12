//! HDR camera with bloom (the Pult glow lives here), pan and zoom.
//!
//! M1 note: plain Bevy input for now; the `bevy_enhanced_input` action maps
//! (GDD §12.2) come with the rebinding UI — tracked as M1 deviation.

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

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
    commands.spawn((
        Camera2d,
        bevy::render::view::Hdr,
        Bloom::default(),
        Projection::Orthographic(OrthographicProjection::default_2d()),
        Transform::from_xyz(0.0, 0.0, 999.0),
        MainCamera,
        Name::new("MainCamera"),
    ));
}

fn pan(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut motion: MessageReader<MouseMotion>,
    time: Res<Time>,
    zoom: Res<Zoom>,
    mut cameras: Query<&mut Transform, With<MainCamera>>,
) {
    let Ok(mut transform) = cameras.single_mut() else {
        return;
    };
    // Drag with middle/right mouse button…
    let mut delta = Vec2::ZERO;
    for event in motion.read() {
        if buttons.pressed(MouseButton::Middle) || buttons.pressed(MouseButton::Right) {
            delta += event.delta;
        }
    }
    transform.translation.x -= delta.x / zoom.0;
    transform.translation.y += delta.y / zoom.0;
    // …or WASD/arrows.
    let mut dir = Vec2::ZERO;
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
    let speed = 600.0 / zoom.0;
    transform.translation.x += dir.x * speed * time.delta_secs();
    transform.translation.y += dir.y * speed * time.delta_secs();
}

fn zoom(
    mut wheel: MessageReader<MouseWheel>,
    mut zoom: ResMut<Zoom>,
    mut cameras: Query<&mut Projection, With<MainCamera>>,
) {
    let mut steps = 0.0;
    for event in wheel.read() {
        steps += event.y;
    }
    if steps == 0.0 {
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
