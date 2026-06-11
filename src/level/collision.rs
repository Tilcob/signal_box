//! Grid collision, built from the LDtk collision catalog on every level load.

mod map;

pub use map::CollisionMap;

use bevy::prelude::*;
use ldtk_integration::{
    CurrentLdtkLevel, LdtkCollider, LdtkLayerType, LdtkLevelReadyEvent, LdtkMapCatalog,
};

/// Circle collider for kinematic actors (player, NPCs).
#[derive(Component, Clone, Copy)]
pub struct Collider {
    pub radius: f32,
    /// Offset of the circle from the entity origin — e.g. a negative Y to put
    /// the collider at the feet of a tall, centre-anchored sprite.
    pub offset: Vec2,
}

impl Collider {
    /// Template API — for actors without a vertical offset.
    #[allow(dead_code)]
    pub fn new(radius: f32) -> Self {
        Self {
            radius,
            offset: Vec2::ZERO,
        }
    }

    pub fn with_offset_y(radius: f32, offset_y: f32) -> Self {
        Self {
            radius,
            offset: Vec2::new(0.0, offset_y),
        }
    }

    /// Collider centre for an entity whose origin is at `origin`.
    pub fn center(&self, origin: Vec2) -> Vec2 {
        origin + self.offset
    }
}

pub struct CollisionPlugin;

impl Plugin for CollisionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingRebuild>().add_systems(
            Update,
            (
                request_collision_rebuild,
                detect_collider_changes,
                rebuild_collision_map,
            )
                .chain(),
        );
    }
}

/// How many frames [`rebuild_collision_map`] keeps retrying after a level
/// became ready. The crate tags solid IntGrid cells with [`LdtkCollider`] a
/// few frames *after* [`LdtkLevelReadyEvent`] — a one-shot rebuild would race
/// that and see zero cells. Levels genuinely without walls give up after this
/// many frames.
const REBUILD_MAX_FRAMES: u32 = 60;

/// Frames to wait before the first build attempt: reading colliders the same
/// frame they spawn would see unpropagated (identity) `GlobalTransform`s.
const REBUILD_MIN_FRAMES: u32 = 2;

/// Level identifier waiting for its collision map, plus retry budget.
#[derive(Resource, Default)]
struct PendingRebuild {
    level: Option<String>,
    frames_waited: u32,
}

/// Queues a collision rebuild whenever a level becomes ready — initial load
/// and every transition. The old map is dropped immediately so the previous
/// level's walls can't block the freshly teleported player.
fn request_collision_rebuild(
    mut ready: MessageReader<LdtkLevelReadyEvent>,
    mut pending: ResMut<PendingRebuild>,
    mut commands: Commands,
) {
    // If several transitions land in one frame only the last one matters.
    let Some(msg) = ready.read().last() else {
        return;
    };
    pending.level = Some(msg.level_identifier.clone());
    pending.frames_waited = 0;
    commands.remove_resource::<CollisionMap>();
}

/// Re-arms the rebuild whenever solid cells spawn or despawn *outside* a level
/// transition — most importantly when the file watcher hot-reloads the `.ldtk`
/// world and `bevy_ecs_ldtk` respawns it (no [`LdtkLevelReadyEvent`] fires
/// then). The old map stays active until the new one is built. Resetting the
/// frame counter on every change also makes the build wait until multi-frame
/// spawning has settled.
fn detect_collider_changes(
    added: Query<(), Added<LdtkCollider>>,
    mut removed: RemovedComponents<LdtkCollider>,
    current: Res<CurrentLdtkLevel>,
    mut pending: ResMut<PendingRebuild>,
) {
    let changed = !added.is_empty() || removed.read().count() > 0;
    if !changed {
        return;
    }
    // Before the first transition completes there is no level identifier yet;
    // the ready event will arm the rebuild in that case.
    let Some(level) = pending.level.take().or_else(|| current.identifier.clone()) else {
        return;
    };
    pending.level = Some(level);
    pending.frames_waited = 0;
}

/// Builds the queued [`CollisionMap`] from the **rendered world positions** of
/// the solid cells the crate tagged with [`LdtkCollider`]. Reading those
/// transforms is the ground truth: the grid is guaranteed to coincide with the
/// tiles on screen and with spawn-point positions, with no LDtk↔Bevy
/// coordinate conversion of our own. Movement keeps working without a map
/// (free move), so a level with zero solid cells is fine.
fn rebuild_collision_map(
    mut pending: ResMut<PendingRebuild>,
    catalog: Res<LdtkMapCatalog>,
    colliders: Query<(&GlobalTransform, &LdtkCollider)>,
    mut commands: Commands,
) {
    let Some(level) = pending.level.clone() else {
        return;
    };
    pending.frames_waited += 1;
    if pending.frames_waited < REBUILD_MIN_FRAMES {
        return;
    }

    let centers: Vec<Vec2> = colliders
        .iter()
        .filter(|(_, collider)| collider.solid)
        .map(|(transform, _)| transform.translation().truncate())
        .collect();

    if centers.is_empty() {
        if pending.frames_waited >= REBUILD_MAX_FRAMES {
            info!("Level '{level}' has no solid IntGrid cells — collision disabled.");
            pending.level = None;
            // Also covers "world unloaded": no colliders left, so the previous
            // map must not keep blocking movement.
            commands.remove_resource::<CollisionMap>();
        }
        return;
    }

    // Tile size from the level's IntGrid layer definition in the catalog.
    let tile_size = catalog
        .layers
        .values()
        .find(|layer| {
            layer.level_identifier == level
                && layer.layer_type == LdtkLayerType::IntGrid
                && layer.grid_size > 0
        })
        .map(|layer| layer.grid_size as f32)
        .unwrap_or_else(|| {
            warn!("No IntGrid layer found for '{level}' — assuming 16px tiles.");
            16.0
        });

    let map = map::build_from_centers(&centers, tile_size);
    info!(
        "CollisionMap for '{level}': {}x{} cells of {}px ({} solid)",
        map.width(),
        map.height(),
        map.tile_size(),
        centers.len()
    );
    commands.insert_resource(map);
    pending.level = None;
}
