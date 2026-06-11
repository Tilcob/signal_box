//! Kinematic movement against the collision map.

use bevy::prelude::*;

use super::{Player, Velocity};
use crate::level::{Collider, CollisionMap};

/// Moves the player by its velocity, sliding along walls via the collision
/// map. Without a map (no level loaded) the player moves freely.
///
/// `pub` so other systems (e.g. the follow camera) can order against it.
pub fn apply_movement(
    time: Res<Time>,
    map: Option<Res<CollisionMap>>,
    mut players: Query<(&Velocity, &Collider, &mut Transform), With<Player>>,
) {
    let Ok((velocity, collider, mut transform)) = players.single_mut() else {
        return;
    };
    if velocity.0 == Vec2::ZERO {
        return;
    }

    let start = transform.translation.truncate();
    let desired = start + velocity.0 * time.delta_secs();

    let resolved = match map {
        // Sweep the collider centre, then map back to the entity origin.
        Some(map) => {
            map.sweep_circle(
                collider.center(start),
                collider.center(desired),
                collider.radius,
            ) - collider.offset
        }
        None => desired,
    };

    transform.translation.x = resolved.x;
    transform.translation.y = resolved.y;
}
