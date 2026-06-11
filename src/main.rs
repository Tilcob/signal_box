//! Composition root: window/asset setup plus one plugin per module group.
//!
//! Run with dev tools (default): `cargo run`
//! Ship a release build:         `cargo build --release --no-default-features`

mod core;
mod graphics;
mod level;
mod player;

#[cfg(feature = "dev")]
mod dev_tools;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

fn main() {
    let mut app = App::new();

    app.insert_resource(ClearColor(Color::srgb(0.08, 0.08, 0.1)))
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    // With the `dev` feature the file watcher hot-reloads every
                    // asset edit (tunables, LDtk worlds, sprites) at runtime.
                    watch_for_changes_override: cfg!(feature = "dev").then_some(true),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "template_2d".into(),
                        resolution: WindowResolution::new(1280, 720),
                        present_mode: PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                })
                // Crisp pixel art: no texture filtering.
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins((
            core::CorePlugin,
            level::LevelPlugin,
            player::PlayerPlugin,
            graphics::GraphicsPlugin,
        ));

    #[cfg(feature = "dev")]
    app.add_plugins(dev_tools::DevToolsPlugin);

    app.run();
}
