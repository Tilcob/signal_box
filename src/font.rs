//! The shared UI font: DejaVu Sans Mono with full Unicode coverage — Bevy's
//! built-in default font is an ASCII-only Fira Mono subset that renders
//! umlauts and ● ○ ✓ → · as tofu boxes.

use bevy::prelude::*;
use bevy::text::Font;

const PATH: &str = "assets/fonts/DejaVuSansMono.ttf";

/// Handle to the UI font, passed explicitly into every `TextFont`.
///
/// Deliberately its OWN asset under its own handle: replacing the asset
/// under the DEFAULT handle corrupts text, because bevy_text loads one face
/// per asset id into cosmic-text exactly once
/// (`map_handle_to_font_id.entry().or_insert`) and glyph atlases are only
/// cleared on `AssetEvent::Removed` — two faces then share one atlas and
/// their glyph ids collide (giant/garbled glyphs).
#[derive(Resource)]
pub struct UiFont(pub Handle<Font>);

pub struct FontPlugin;

impl Plugin for FontPlugin {
    fn build(&self, app: &mut App) {
        // Inserted at plugin-BUILD time, not via a startup system: bevy_state
        // runs the initial GameState transition during PreStartup as well,
        // so `OnEnter(LevelSelect)` spawns text in the same schedule a
        // PreStartup system would race against. Building the resource here
        // guarantees it exists before any schedule runs at all.
        let handle = match std::fs::read(PATH)
            .map_err(|e| e.to_string())
            .and_then(|bytes| Font::try_from_bytes(bytes).map_err(|e| e.to_string()))
        {
            Ok(font) => app
                .world_mut()
                .resource_mut::<Assets<Font>>()
                .add(font),
            Err(e) => {
                warn!("{PATH} unusable ({e}) — falling back to the built-in ASCII font");
                Handle::default()
            }
        };
        app.insert_resource(UiFont(handle));
    }
}
