//! Stellwerk — M1 vertical slice (plan: plans/M1/M1-vertical-slice.md).
//!
//! Composition root ONLY: window setup and one plugin per module. The
//! simulation lives entirely in `stellwerk_sim`; this app is editor UX,
//! rendering and UI on top of its public API (GDD §12.1).

mod board;
mod camera;
mod editor;
mod font;
mod i18n;
mod levels;
mod run;
mod state;
mod ui;

#[cfg(feature = "dev")]
mod dev_tools;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowMode};

fn main() {
    let mut app = App::new();

    app.insert_resource(ClearColor(Color::srgb(0.013, 0.016, 0.022)))
        .add_plugins(DefaultPlugins.set(
            WindowPlugin {
                primary_window: Some(Window {
                    title: "Stellwerk".into(),
                    mode: WindowMode::BorderlessFullscreen(MonitorSelection::Current),
                    present_mode: PresentMode::AutoVsync,
                    ..default()
                }),
                ..default()
            }
        ))
        .add_plugins((
            font::FontPlugin,
            state::StatePlugin,
            levels::LevelsPlugin,
            camera::CameraPlugin,
            board::BoardPlugin,
            editor::EditorPlugin,
            run::RunPlugin,
            ui::UiPlugin,
        ));

    #[cfg(feature = "dev")]
    app.add_plugins(dev_tools::DevToolsPlugin);

    app.run();
}
