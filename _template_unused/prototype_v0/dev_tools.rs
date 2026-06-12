//! Dev-only tooling — compiled only with the `dev` feature (on by default).
//!
//! | Key   | Tool                                                       |
//! |-------|------------------------------------------------------------|
//! | F3    | FPS overlay                                                |
//! | F11   | Tunables inspector (temporary runtime tweaks)              |
//! | F12   | World inspector (entities, components, resources, assets)  |
//!
//! Asset hot reload needs no key: with the `dev` feature every save of
//! `assets/config/game.tunables.ron` is applied to the running game.

use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin};
use bevy::input::common_conditions::input_toggle_active;
use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::{ResourceInspectorPlugin, WorldInspectorPlugin};

use crate::core::Tunables;

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
            .add_systems(Update, toggle_fps_overlay);
    }
}

fn toggle_fps_overlay(input: Res<ButtonInput<KeyCode>>, mut config: ResMut<FpsOverlayConfig>) {
    if input.just_pressed(KeyCode::F3) {
        config.enabled = !config.enabled;
    }
}
