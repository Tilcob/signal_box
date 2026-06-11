//! LDtk world loading + level management via `ldtk_integration`.
//!
//! The crate does the heavy lifting: it loads [`WORLD_PATH`], auto-promotes the
//! first spawned level into a full transition, resolves spawn points, teleports
//! the entity marked [`ldtk_integration::LdtkLevelPlayer`] and cleans up
//! level-scoped entities on every switch. This module only supplies the
//! configuration and registers the entity identifiers used in the LDtk editor.
//!
//! Switching levels from gameplay code:
//! ```ignore
//! use ldtk_integration::LdtkCommandExt;
//! commands.transition_to_ldtk_level("Level_1", Some(PLAYER_SPAWN_TAG.into()));
//! ```

pub mod collision;

pub use collision::{Collider, CollisionMap};

use bevy::prelude::*;
use ldtk_integration::{
    GameLdtkPlugin, LdtkAppExt, LdtkConfig, LdtkLevelManagerConfig, LdtkLevelReadyEvent,
    LevelManagerPlugin,
};

/// Asset-relative path of the LDtk world. Drop your world file here (or change
/// the path). Until the file exists the app still runs; no level loads.
pub const WORLD_PATH: &str = "levels/world.ldtk";

/// IntGrid values that count as solid walls. Everything else (floor variants
/// etc.) stays walkable. Must match the IntGrid layer in the LDtk editor.
pub const SOLID_INT_GRID_VALUES: [i32; 1] = [1];

/// LDtk entity identifier (and tag) for player spawn points.
pub const PLAYER_SPAWN_TAG: &str = "PlayerSpawn";

/// Marker attached to LDtk `PlayerSpawn` entities. The level manager resolves
/// spawn points by identifier/tag on its own; the marker is for game code that
/// wants to query spawn points directly.
#[derive(Component, Default)]
pub struct PlayerSpawnPoint;

#[derive(Bundle, Default)]
pub struct PlayerSpawnBundle {
    spawn_point: PlayerSpawnPoint,
}

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(GameLdtkPlugin::new(
            LdtkConfig::default()
                .with_world_asset_path(WORLD_PATH)
                .with_solid_int_grid_values(SOLID_INT_GRID_VALUES),
        ))
        .add_plugins(LevelManagerPlugin)
        .add_plugins(collision::CollisionPlugin)
        .insert_resource(LdtkLevelManagerConfig {
            default_spawn_tag: PLAYER_SPAWN_TAG.to_string(),
            default_spawn_identifier: PLAYER_SPAWN_TAG.to_string(),
            // Template-friendly: a level without a spawn point falls back to
            // (0,0) instead of failing the transition.
            allow_missing_spawnpoints: true,
            ..default()
        })
        // Register the LDtk entity identifiers placed in levels. Add one line
        // per new entity type:
        //   .register_ldtk_entity::<MyBundle>("MyIdentifier")
        .register_ldtk_entity::<PlayerSpawnBundle>(PLAYER_SPAWN_TAG)
        .add_systems(Update, log_level_ready);
    }
}

fn log_level_ready(mut ready: MessageReader<LdtkLevelReadyEvent>) {
    for msg in ready.read() {
        info!(
            "Level '{}' ready (player spawn at {:.0?})",
            msg.level_identifier, msg.position
        );
    }
}
