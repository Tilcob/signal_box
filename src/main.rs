//! Stellwerk — M1 vertical slice (plan: plans/M1/M1-vertical-slice.md).
//!
//! Composition root ONLY: window setup and one plugin per module. The
//! simulation lives entirely in `stellwerk_sim`; this app is editor UX,
//! rendering and UI on top of its public API (GDD §12.1).

mod board;
mod camera;
mod clipboard;
mod editor;
mod font;
mod i18n;
mod levels;
mod loading;
mod run;
mod state;
mod ui;

#[cfg(feature = "dev")]
mod authoring;
#[cfg(feature = "dev")]
mod dev_tools;
mod audio;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowMode, WindowPosition};

fn main() {
    let mut app = App::new();

    // Debug aid: STELLWERK_WINDOWED=1 starts in a fixed window on the
    // primary monitor instead of borderless fullscreen — fullscreen on
    // multi-monitor setups is awkward for screenshots and automation.
    let windowed = std::env::var_os("STELLWERK_WINDOWED").is_some();
    let window = if windowed {
        Window {
            title: "Stellwerk".into(),
            mode: WindowMode::Windowed,
            position: WindowPosition::At(IVec2::new(80, 80)),
            resolution: (1600, 900).into(),
            present_mode: PresentMode::AutoVsync,
            ..default()
        }
    } else {
        Window {
            title: "Stellwerk".into(),
            mode: WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            present_mode: PresentMode::AutoVsync,
            ..default()
        }
    };

    app.insert_resource(ClearColor(Color::srgb(0.013, 0.016, 0.022)))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(window),
                    ..default()
                })
                // bevy_kira_audio drives the audio device; disable Bevy's own
                // audio plugin so the two don't both grab the output.
                .disable::<bevy::audio::AudioPlugin>(),
        )
        .add_plugins((
            font::FontPlugin,
            state::StatePlugin,
            levels::LevelsPlugin,
            loading::LoadingPlugin,
            camera::CameraPlugin,
            board::BoardPlugin,
            editor::EditorPlugin,
            run::RunPlugin,
            ui::UiPlugin,
            audio::AudioManagerPlugin,
        ));

    #[cfg(feature = "dev")]
    app.add_plugins(dev_tools::DevToolsPlugin);

    app.run();
}
