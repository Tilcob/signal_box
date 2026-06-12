//! Dev-only tooling (feature `dev`): F12 world inspector, F3 FPS overlay.

use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig};
use bevy::input::common_conditions::input_toggle_active;
use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;

pub struct DevToolsPlugin;

impl Plugin for DevToolsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .add_plugins(
                WorldInspectorPlugin::new().run_if(input_toggle_active(false, KeyCode::F12)),
            )
            .add_plugins(FpsOverlayPlugin {
                config: FpsOverlayConfig {
                    enabled: false,
                    // The graph has its own flag and DEFAULTS TO ON — without
                    // this it renders its red/orange bars in the top-left
                    // corner even though the overlay is "disabled".
                    frame_time_graph_config: FrameTimeGraphConfig {
                        enabled: false,
                        ..default()
                    },
                    ..default()
                },
            })
            .add_systems(Update, toggle_fps);
    }
}

fn toggle_fps(input: Res<ButtonInput<KeyCode>>, mut config: ResMut<FpsOverlayConfig>) {
    if input.just_pressed(KeyCode::F3) {
        config.enabled = !config.enabled;
        config.frame_time_graph_config.enabled = config.enabled;
    }
}
