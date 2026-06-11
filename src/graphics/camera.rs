//! Smooth follow camera: dead-zone, movement lead, live-tunable zoom.

use bevy::prelude::*;

use crate::core::{GameplaySet, Tunables};
use crate::player::{Player, Velocity};

const CAMERA_Z: f32 = 999.0;

#[derive(Component)]
pub struct MainCamera;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera)
            .add_systems(
                Update,
                // Follow *after* the player moved this frame, so the camera
                // reads the up-to-date position instead of lagging one frame.
                follow_player
                    .after(crate::player::apply_movement)
                    .in_set(GameplaySet),
            )
            .add_systems(
                Update,
                apply_zoom.run_if(resource_changed::<Tunables>),
            );
    }
}

/// An orthographic `scale` < 1 shows fewer world units on screen = magnified,
/// so the zoom factor is applied as its reciprocal.
fn spawn_camera(mut commands: Commands, tunables: Res<Tunables>) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scale: 1.0 / tunables.camera.zoom.max(0.01),
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 0.0, CAMERA_Z),
        MainCamera,
        Name::new("MainCamera"),
    ));
}

/// Applies zoom changes from hot-reloaded / inspector-tweaked tunables live.
fn apply_zoom(tunables: Res<Tunables>, mut cameras: Query<&mut Projection, With<MainCamera>>) {
    let Ok(mut projection) = cameras.single_mut() else {
        return;
    };
    if let Projection::Orthographic(ortho) = &mut *projection {
        ortho.scale = 1.0 / tunables.camera.zoom.max(0.01);
    }
}

/// Dead-zone follow: the player moves freely inside a box around the camera;
/// once outside, the camera lerps so the player sits at the box edge, leading
/// slightly in the movement direction.
fn follow_player(
    time: Res<Time>,
    tunables: Res<Tunables>,
    players: Query<(&Transform, &Velocity), With<Player>>,
    mut cameras: Query<&mut Transform, (With<MainCamera>, Without<Player>)>,
) {
    let Ok((player_transform, velocity)) = players.single() else {
        return;
    };
    let Ok(mut camera_transform) = cameras.single_mut() else {
        return;
    };

    let cam = &tunables.camera;
    let player_pos = player_transform.translation.truncate();
    let camera_pos = camera_transform.translation.truncate();
    let delta = player_pos - camera_pos;

    let out_x = delta.x.abs() > cam.deadzone_x;
    let out_y = delta.y.abs() > cam.deadzone_y;
    if !out_x && !out_y {
        return;
    }

    // Absolute target: player at the dead-zone edge on each exceeded axis.
    let mut target = camera_pos;
    if out_x {
        target.x = player_pos.x - cam.deadzone_x * delta.x.signum();
    }
    if out_y {
        target.y = player_pos.y - cam.deadzone_y * delta.y.signum();
    }
    target += velocity.normalize_or_zero() * cam.lead;

    let lerp_factor = (cam.lerp_speed * time.delta_secs()).clamp(0.0, 1.0);
    let new_pos = camera_pos.lerp(target, lerp_factor);

    // Round to whole world units to reduce tile shimmer; the camera still
    // follows smoothly because the lerp target stays sub-pixel.
    camera_transform.translation.x = new_pos.x.round();
    camera_transform.translation.y = new_pos.y.round();
}
