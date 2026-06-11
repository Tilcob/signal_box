//! Player entity setup + spawn safety nets.

use bevy::prelude::*;
use ldtk_integration::LdtkLevelPlayer;

use super::{Player, Velocity};
use crate::core::Tunables;
use crate::graphics::y_sort::YSort;
use crate::level::{Collider, CollisionMap};

/// Initial Z; overwritten every frame by the Y-sort system.
const PLAYER_Z: f32 = 10.0;

/// Spawns the player once with a placeholder sprite. Swap the `Sprite` for
/// `Sprite::from_atlas_image(..)` plus a
/// [`crate::graphics::animation::SpriteAnimation`] when real art lands.
pub(super) fn spawn_player(mut commands: Commands, tunables: Res<Tunables>) {
    commands.spawn((
        Player,
        // The level manager teleports this entity on every level transition.
        LdtkLevelPlayer,
        Sprite {
            color: Color::srgb(0.25, 0.8, 1.0),
            custom_size: Some(Vec2::new(12.0, 16.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, PLAYER_Z),
        Velocity::default(),
        Collider::with_offset_y(
            tunables.player.collider_radius,
            tunables.player.collider_offset_y,
        ),
        YSort::default(),
        Name::new("Player"),
    ));
}

/// Keeps the collider in sync with live tunables tweaks.
pub(super) fn sync_collider_from_tunables(
    tunables: Res<Tunables>,
    mut players: Query<&mut Collider, With<Player>>,
) {
    for mut collider in &mut players {
        collider.radius = tunables.player.collider_radius;
        collider.offset.y = tunables.player.collider_offset_y;
    }
}

/// Safety net after each level load: if the spawn position overlaps a wall
/// (authoring mistake, fallback spawn at (0,0)), snap to the nearest clear tile.
pub(super) fn snap_out_of_walls(
    map: Res<CollisionMap>,
    mut players: Query<(&mut Transform, &Collider), With<Player>>,
) {
    let Ok((mut transform, collider)) = players.single_mut() else {
        return;
    };
    let center = collider.center(transform.translation.truncate());
    if map.is_circle_clear(center, collider.radius) {
        return;
    }
    if let Some(clear) = map.find_nearest_clear_position(center, collider.radius) {
        let origin = clear - collider.offset;
        transform.translation.x = origin.x;
        transform.translation.y = origin.y;
        warn!("Player spawned inside a wall — snapped to nearest clear position {clear:.0?}");
    } else {
        warn!("Player is inside a wall and no clear position was found nearby.");
    }
}
