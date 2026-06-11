//! Dev-only tooling — compiled only with the `dev` feature (on by default).
//!
//! | Key   | Tool                                                        |
//! |-------|-------------------------------------------------------------|
//! | F1    | Debug overlay: collision gizmos + level-switch number keys  |
//! | F3    | FPS overlay                                                 |
//! | F11   | Tunables inspector (temporary runtime tweaks)               |
//! | F12   | World inspector (entities, components, resources, assets)  |
//! | 1–9   | While F1 is on: switch to `Level_0` … `Level_8`             |
//!
//! Asset hot reload needs no key: with the `dev` feature every saved asset —
//! including `assets/config/game.tunables.ron` and the LDtk world — is applied
//! to the running game automatically.

use bevy::color::palettes::css::{LIME, RED};
use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin};
use bevy::input::common_conditions::input_toggle_active;
use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::{ResourceInspectorPlugin, WorldInspectorPlugin};
use ldtk_integration::LdtkCommandExt;

use crate::core::Tunables;
use crate::level::{Collider, CollisionMap, PLAYER_SPAWN_TAG};

/// F1 master switch for the in-world debug drawing + level-switch keys.
#[derive(Resource, Default)]
struct DebugOverlay(bool);

pub struct DevToolsPlugin;

impl Plugin for DevToolsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .add_plugins(
                WorldInspectorPlugin::new().run_if(input_toggle_active(false, KeyCode::F12)),
            )
            .add_plugins(
                ResourceInspectorPlugin::<Tunables>::new()
                    .run_if(input_toggle_active(false, KeyCode::F11)),
            )
            .add_plugins(FpsOverlayPlugin {
                config: FpsOverlayConfig {
                    enabled: false,
                    ..default()
                },
            })
            .init_resource::<DebugOverlay>()
            .add_systems(
                Update,
                (
                    toggle_debug_overlay,
                    toggle_fps_overlay,
                    (draw_collision_gizmos, debug_level_switch).run_if(overlay_active),
                ),
            );
    }
}

fn overlay_active(overlay: Res<DebugOverlay>) -> bool {
    overlay.0
}

fn toggle_debug_overlay(input: Res<ButtonInput<KeyCode>>, mut overlay: ResMut<DebugOverlay>) {
    if input.just_pressed(KeyCode::F1) {
        overlay.0 = !overlay.0;
        info!(
            "Debug overlay {} (collision gizmos, level-switch keys 1-9)",
            if overlay.0 { "ON" } else { "OFF" }
        );
    }
}

fn toggle_fps_overlay(input: Res<ButtonInput<KeyCode>>, mut config: ResMut<FpsOverlayConfig>) {
    if input.just_pressed(KeyCode::F3) {
        config.enabled = !config.enabled;
    }
}

/// Solid tiles as red squares, every `Collider` as a green circle.
fn draw_collision_gizmos(
    mut gizmos: Gizmos,
    map: Option<Res<CollisionMap>>,
    actors: Query<(&Transform, &Collider)>,
) {
    if let Some(map) = map {
        let size = Vec2::splat(map.tile_size());
        for center in map.solid_world_centers() {
            gizmos.rect_2d(Isometry2d::from_translation(center), size, RED.with_alpha(0.6));
        }
    }
    for (transform, collider) in &actors {
        gizmos.circle_2d(
            Isometry2d::from_translation(collider.center(transform.translation.truncate())),
            collider.radius,
            LIME,
        );
    }
}

/// Number key `N` switches to `Level_{N-1}` (so `1` → `Level_0`).
fn debug_level_switch(input: Res<ButtonInput<KeyCode>>, mut commands: Commands) {
    const KEYS: [KeyCode; 9] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    for (i, key) in KEYS.iter().enumerate() {
        if input.just_pressed(*key) {
            let level = format!("Level_{i}");
            info!("Debug: switching to level '{level}'");
            commands.transition_to_ldtk_level(level, Some(PLAYER_SPAWN_TAG.to_string()));
            break;
        }
    }
}
