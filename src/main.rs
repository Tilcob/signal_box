//! Stellwerk — the Bevy frontend.
//!
//! Composition root ONLY: window setup and one plugin per module. The
//! simulation lives entirely in `stellwerk_sim`; this app is editor UX,
//! rendering and UI on top of its public API.

mod board;
mod camera;
mod clipboard;
mod console;
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
use bevy::window::PresentMode;
#[cfg(not(target_arch = "wasm32"))]
use bevy::window::{WindowMode, WindowPosition};

fn main() {
    let mut app = App::new();

    // Desktop: borderless fullscreen, or a fixed window with STELLWERK_WINDOWED=1
    // (fullscreen on multi-monitor setups is awkward for screenshots/automation).
    #[cfg(not(target_arch = "wasm32"))]
    let window = {
        let windowed = std::env::var_os("STELLWERK_WINDOWED").is_some();
        if windowed {
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
        }
    };

    // Browser: render into <canvas id="bevy"> and follow its parent's size.
    #[cfg(target_arch = "wasm32")]
    let window = Window {
        title: "Stellwerk".into(),
        canvas: Some("#bevy".into()),
        fit_canvas_to_parent: true,
        present_mode: PresentMode::AutoVsync,
        ..default()
    };

    app.insert_resource(ClearColor(Color::srgb(0.013, 0.016, 0.022)))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(window),
                    ..default()
                })
                // wasm: dev/static servers answer a missing `<asset>.meta` with
                // index.html (200), which Bevy then fails to parse as asset meta
                // and the whole asset load fails — that is why every audio file
                // errored out. Skip meta lookups entirely in the browser. On
                // desktop this is `AssetPlugin::default()`, i.e. unchanged.
                .set(AssetPlugin {
                    #[cfg(target_arch = "wasm32")]
                    meta_check: bevy::asset::AssetMetaCheck::Never,
                    ..default()
                })
                // Mirror our own `info!`/`warn!`/`error!` (incl. dev) into the
                // in-level console; engine noise is filtered out by target.
                .set(bevy::log::LogPlugin {
                    custom_layer: console::console_layer,
                    ..default()
                })
                // bevy_kira_audio drives the audio device; disable Bevy's own
                // audio plugin so the two don't both grab the output.
                .disable::<bevy::audio::AudioPlugin>(),
        )
        .add_plugins((
            font::FontPlugin,
            state::StatePlugin,
            console::ConsolePlugin,
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
