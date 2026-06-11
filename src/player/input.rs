//! Keyboard input → [`Velocity`].

use bevy::prelude::*;

use super::{Player, Velocity};
use crate::core::Tunables;

/// WASD / arrow keys to move, hold Shift to run.
pub(super) fn read_movement_input(
    input: Res<ButtonInput<KeyCode>>,
    tunables: Res<Tunables>,
    mut players: Query<&mut Velocity, With<Player>>,
) {
    let Ok(mut velocity) = players.single_mut() else {
        return;
    };

    let mut direction = Vec2::ZERO;
    if input.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        direction.y += 1.0;
    }
    if input.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        direction.y -= 1.0;
    }
    if input.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        direction.x -= 1.0;
    }
    if input.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        direction.x += 1.0;
    }

    let speed = if input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
        tunables.player.run_speed
    } else {
        tunables.player.walk_speed
    };

    velocity.0 = direction.normalize_or_zero() * speed;
}
