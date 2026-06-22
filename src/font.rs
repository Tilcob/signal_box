//! The shared UI font: Saira Semi Condensed (DIN-like signage grotesque) —
//! Bevy's built-in default is an ASCII-only Fira Mono subset that
//! renders umlauts as tofu. Saira covers Latin + ß but has no symbol glyphs;
//! the UI's status icons (medals, solved, errors) are drawn as UI shapes
//! (`widgets::dot`) or use punctuation (»«×).

use bevy::prelude::*;
use bevy::text::Font;

/// The shipped UI face. Swapping it (e.g. to another DIN-like signage font)
/// is a one-liner here plus dropping the new `.ttf`
/// and its OFL/Apache/PD license beside it — the `shipped_font_covers_all_ui_glyphs`
/// test then guarantees the replacement still renders every UI character.
/// A proportional face must keep tabular figures (or the read-only schedule's
/// `·`-separated columns lose their alignment).
const PATH: &str = "assets/fonts/Saira_Semi_Condensed/SairaSemiCondensed-Regular.ttf";

/// Embedded copy for wasm (the browser has no filesystem). Must stay in sync
/// with `PATH`; `include_bytes!` needs a literal, so the path is spelled out.
#[cfg(target_arch = "wasm32")]
const FONT_BYTES: &[u8] =
    include_bytes!("../assets/fonts/Saira_Semi_Condensed/SairaSemiCondensed-Regular.ttf");

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
        // Desktop reads the font from disk (swappable without a rebuild); wasm
        // uses the embedded copy, since the browser has no filesystem.
        #[cfg(not(target_arch = "wasm32"))]
        let bytes = std::fs::read(PATH).map_err(|e| e.to_string());
        #[cfg(target_arch = "wasm32")]
        let bytes: Result<Vec<u8>, String> = Ok(FONT_BYTES.to_vec());

        let handle = match bytes
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

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    /// Symbols the UI draws from source string literals, i.e. NOT via the i18n
    /// tables (separators, arrows, marks). Keep in sync when a new symbol is
    /// added to the UI — same `#[cfg(test)]` const pattern as the i18n
    /// decode-error keys.
    // ●○✓ are drawn as UI shapes now, ▶◀✗ replaced by »«×,
    // so the font only needs these punctuation/arrow symbols.
    const UI_GLYPHS: &str = "·→×…»≈";

    fn i18n_chars(lang: &str) -> BTreeSet<char> {
        let path = format!("{}/assets/i18n/{lang}.ron", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
        let map: BTreeMap<String, String> =
            ron::from_str(&text).unwrap_or_else(|e| panic!("parse {path}: {e}"));
        map.values().flat_map(|v| v.chars()).collect()
    }

    /// The font-coverage check: the shipped font must
    /// render every character the UI can display — both i18n tables (incl. all
    /// umlauts and ß) plus the hardcoded symbols — otherwise it shows tofu.
    #[test]
    fn shipped_font_covers_all_ui_glyphs() {
        let font_path = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), super::PATH);
        let data = std::fs::read(&font_path).unwrap_or_else(|e| panic!("read {font_path}: {e}"));
        let face = ttf_parser::Face::parse(&data, 0).expect("shipped font parses");

        let mut required = i18n_chars("en");
        required.extend(i18n_chars("de"));
        required.extend(UI_GLYPHS.chars());

        let missing: Vec<char> = required
            .into_iter()
            .filter(|&c| !c.is_control() && face.glyph_index(c).is_none())
            .collect();

        assert!(
            missing.is_empty(),
            "shipped font {} lacks glyphs for: {missing:?}",
            super::PATH
        );
    }
}
