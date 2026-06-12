//! Stellwerk — M1 vertical slice (plan: plans/M1/M1-vertical-slice.md).
//!
//! Composition root: window, HDR camera + bloom (the Pult glow), one plugin
//! per module. The simulation lives entirely in `stellwerk_sim`; this app is
//! editor UX, rendering and UI on top of its public API (GDD §12.1).

mod board;
mod camera;
mod editor;
mod i18n;
mod levels;
mod run;
mod state;
mod ui;

#[cfg(feature = "dev")]
mod dev_tools;

use bevy::prelude::*;
use bevy::text::Font;
use bevy::window::{PresentMode, WindowMode};

/// Replaces Bevy's built-in default font (an ASCII-only Fira Mono subset)
/// with a full-coverage one. The UI is German-first (umlauts) and uses
/// symbols like ● ○ ✓ → · — with the subset all of these render as tofu
/// boxes, and the per-frame HUD texts visibly corrupt the glyph atlas.
/// Installing under the default handle means every `TextFont` keeps working.
fn install_ui_font(mut fonts: ResMut<Assets<Font>>) {
    const PATH: &str = "assets/fonts/DejaVuSansMono.ttf";
    match std::fs::read(PATH) {
        Ok(bytes) => match Font::try_from_bytes(bytes) {
            Ok(font) => {
                if let Err(e) = fonts.insert(&Handle::<Font>::default(), font) {
                    warn!("cannot install {PATH} as default font: {e}");
                }
            }
            Err(e) => warn!("{PATH} is not a usable font: {e}"),
        },
        Err(e) => warn!("{PATH} missing ({e}) — non-ASCII glyphs will render as boxes"),
    }
}

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
        .add_systems(Startup, install_ui_font)
        .add_plugins((
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
