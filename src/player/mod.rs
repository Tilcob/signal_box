//! Player: spawn, input and kinematic movement with wall sliding.
//!
//! The entity carries [`ldtk_integration::LdtkLevelPlayer`], so the level
//! manager teleports it to the resolved spawn point after every level
//! transition — no spawn-point code needed here. Speeds and collider come from
//! the hot-reloadable [`Tunables`].

mod input;
mod movement;
mod spawn;

pub use movement::apply_movement;

use bevy::prelude::*;

use crate::core::{GameplaySet, Tunables};
use crate::level::CollisionMap;

#[derive(Component)]
pub struct Player;

/// Velocity in world units per second. Input writes it, movement applies it —
/// keeping the two apart means AI or knockback can drive the same movement
/// system later.
#[derive(Component, Debug, Clone, Copy, Default, Deref, DerefMut)]
pub struct Velocity(pub Vec2);

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn::spawn_player)
            .add_systems(
                Update,
                (input::read_movement_input, movement::apply_movement)
                    .chain()
                    .in_set(GameplaySet),
            )
            .add_systems(
                Update,
                (
                    spawn::sync_collider_from_tunables.run_if(resource_changed::<Tunables>),
                    // A freshly built map means a level was just entered: make
                    // sure the player isn't stuck inside a wall.
                    spawn::snap_out_of_walls
                        .run_if(resource_exists_and_changed::<CollisionMap>),
                ),
            );
    }
}
