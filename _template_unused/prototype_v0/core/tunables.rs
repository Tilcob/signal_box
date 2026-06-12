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

#[derive(Resource, Asset, Reflect, Clone, Debug, Deserialize)]
#[reflect(Resource)]
#[serde(default)]
pub struct Tunables {
    /// Train cruise speed in world units (pixels) per second.
    pub train_speed: f32,
    /// Two trains closer than this (center to center) collide.
    pub collision_distance: f32,
    /// How close a click must be to a signal or switch to toggle it.
    pub click_radius: f32,
    /// A departure is held back while another train is within this distance
    /// of the source node, so trains never spawn on top of each other.
    pub spawn_clearance: f32,
}

impl Default for Tunables {
    fn default() -> Self {
        Self {
            train_speed: 120.0,
            collision_distance: 20.0,
            click_radius: 26.0,
            spawn_clearance: 70.0,
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
