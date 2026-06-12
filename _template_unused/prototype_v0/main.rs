//! Stellwerk prototype — a Zachlike about routing trains with switches and
//! signals. Bring every train of the timetable to the station matching its
//! cargo color, without collisions.
//!
//! Run with dev tools (default): `cargo run`
//! Ship a release build:         `cargo build --release --no-default-features`

mod core;
mod interaction;
mod render;
mod sim;
mod ui;

#[cfg(feature = "dev")]
mod dev_tools;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

fn main() {
    let mut app = App::new();

    app.insert_resource(ClearColor(Color::srgb(0.07, 0.08, 0.10)))
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    // With the `dev` feature the file watcher hot-reloads the
                    // tunables RON file at runtime.
                    watch_for_changes_override: cfg!(feature = "dev").then_some(true),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Stellwerk — signal_box".into(),
                        resolution: WindowResolution::new(1280, 720),
                        present_mode: PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins((
            core::CorePlugin,
            sim::SimPlugin,
            interaction::InteractionPlugin,
            render::RenderPlugin,
            ui::UiPlugin,
        ));

    #[cfg(feature = "dev")]
    app.add_plugins(dev_tools::DevToolsPlugin);

    app.run();
}
