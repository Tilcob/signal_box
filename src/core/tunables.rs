//! Hot-reloadable gameplay constants.
//!
//! Every "magic number" lives in `assets/config/game.tunables.ron` and is
//! mirrored into the [`Tunables`] resource. With the `dev` feature the file
//! watcher re-applies the file on every save, so values can be tuned while the
//! game runs — no recompile. The dev inspector (F11) additionally allows
//! temporary in-memory tweaks; the next file save overwrites them.
//!
//! Adding a constant = add a field here (with a `Default` value) + the entry in
//! the RON file. Fields are `#[serde(default)]`, so old RON files keep working.

use bevy::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use serde::Deserialize;

/// Asset-relative path of the tunables file. The `.tunables.ron` double
/// extension is what routes the file to this loader (Bevy matches everything
/// after the first dot), so plain `.ron` files for other asset types don't
/// conflict.
pub const TUNABLES_PATH: &str = "config/game.tunables.ron";

#[derive(Resource, Asset, Reflect, Clone, Debug, Default, Deserialize)]
#[reflect(Resource)]
#[serde(default)]
pub struct Tunables {
    pub player: PlayerTunables,
    pub camera: CameraTunables,
}

#[derive(Reflect, Clone, Debug, Deserialize)]
#[serde(default)]
pub struct PlayerTunables {
    /// Walk speed in world units per second.
    pub walk_speed: f32,
    /// Speed while Shift is held.
    pub run_speed: f32,
    /// Radius of the player's collision circle.
    pub collider_radius: f32,
    /// Vertical collider offset from the sprite origin; negative moves it down
    /// to the feet of tall, centre-anchored sprites.
    pub collider_offset_y: f32,
}

impl Default for PlayerTunables {
    fn default() -> Self {
        Self {
            walk_speed: 100.0,
            run_speed: 160.0,
            collider_radius: 6.0,
            collider_offset_y: 0.0,
        }
    }
}

#[derive(Reflect, Clone, Debug, Deserialize)]
#[serde(default)]
pub struct CameraTunables {
    /// Magnification: 3.0 shows the world at 3× pixel size.
    pub zoom: f32,
    /// How quickly the camera catches up (1/seconds; higher = snappier).
    pub lerp_speed: f32,
    /// How far the camera leads ahead in the movement direction.
    pub lead: f32,
    /// Half-size of the box (in world units) the player can move in without
    /// the camera following.
    pub deadzone_x: f32,
    pub deadzone_y: f32,
}

impl Default for CameraTunables {
    fn default() -> Self {
        Self {
            zoom: 3.0,
            lerp_speed: 3.0,
            lead: 40.0,
            deadzone_x: 50.0,
            deadzone_y: 50.0,
        }
    }
}

pub struct TunablesPlugin;

impl Plugin for TunablesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RonAssetPlugin::<Tunables>::new(&["tunables.ron"]))
            .register_type::<Tunables>()
            // Live with defaults until the RON file arrives (assets load async),
            // so every system can read `Res<Tunables>` unconditionally.
            .init_resource::<Tunables>()
            .add_systems(Startup, load_tunables)
            .add_systems(Update, apply_tunables);
    }
}

/// Keeps the asset handle alive; dropping it would unload the asset and kill
/// hot reload.
#[derive(Resource)]
struct TunablesHandle(Handle<Tunables>);

fn load_tunables(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(TunablesHandle(asset_server.load(TUNABLES_PATH)));
}

/// Copies the asset into the live resource on first load and on every hot
/// reload. A RON file with a parse error never reaches this point — Bevy logs
/// the loader error and the last good values stay active.
fn apply_tunables(
    mut events: MessageReader<AssetEvent<Tunables>>,
    handle: Res<TunablesHandle>,
    assets: Res<Assets<Tunables>>,
    mut tunables: ResMut<Tunables>,
) {
    for event in events.read() {
        let (AssetEvent::Added { id } | AssetEvent::Modified { id }) = event else {
            continue;
        };
        if *id == handle.0.id()
            && let Some(loaded) = assets.get(*id)
        {
            *tunables = loaded.clone();
            info!("Tunables (re)loaded from `{TUNABLES_PATH}`");
        }
    }
}
