//! i18n: all UI strings come from RON tables under `assets/i18n/`
//! (`de.ron`, `en.ron`). [`t`] looks up the active language with fallback
//! chain: active table → German table → the raw key (visible programmer
//! error, never a panic). The language toggle lives in the level select;
//! the choice persists in the progress file.

use std::collections::BTreeMap;
use std::sync::RwLock;
use stellwerk_sim::grid::Dir8;

static TABLE: RwLock<Option<Table>> = RwLock::new(None);

struct Table {
    active: BTreeMap<String, String>,
    fallback: BTreeMap<String, String>,
}

fn load_table(lang: &str) -> BTreeMap<String, String> {
    let path = format!("assets/i18n/{lang}.ron");
    match std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|text| {
            ron::from_str::<BTreeMap<String, String>>(&text).map_err(|e| e.to_string())
        }) {
        Ok(map) => map,
        Err(e) => {
            eprintln!("i18n: {path} unreadable: {e}");
            BTreeMap::new()
        }
    }
}

/// Switches the active language ("de" / "en") and (re)loads the tables.
pub fn set_lang(lang: &str) {
    let fallback = load_table("de");
    let active = if lang == "de" {
        fallback.clone()
    } else {
        load_table(lang)
    };
    *TABLE.write().expect("i18n lock") = Some(Table { active, fallback });
}

/// Localized compass label of a connector direction ("O" in German is "E"
/// in English) — used at switch exits and in the switch panel.
pub fn dir_label(dir: Dir8) -> String {
    t(match dir {
        Dir8::N => "dir.N",
        Dir8::NE => "dir.NE",
        Dir8::E => "dir.E",
        Dir8::SE => "dir.SE",
        Dir8::S => "dir.S",
        Dir8::SW => "dir.SW",
        Dir8::W => "dir.W",
        Dir8::NW => "dir.NW",
    })
}

pub fn t(key: &str) -> String {
    let guard = TABLE.read().expect("i18n lock");
    if let Some(table) = guard.as_ref() {
        if let Some(value) = table.active.get(key) {
            return value.clone();
        }
        if let Some(value) = table.fallback.get(key) {
            return value.clone();
        }
    }
    key.to_string()
}

/// Like [`t`], but on a missing key returns `fallback` instead of the key
/// itself — for data-driven strings (level names, station labels) whose
/// authored (German) value is the natural fallback.
pub fn t_or(key: &str, fallback: &str) -> String {
    let guard = TABLE.read().expect("i18n lock");
    if let Some(table) = guard.as_ref() {
        if let Some(value) = table.active.get(key) {
            return value.clone();
        }
        if let Some(value) = table.fallback.get(key) {
            return value.clone();
        }
    }
    fallback.to_string()
}

/// Localized level name (fallback = authored `level.name`). `id` is the level
/// file stem, e.g. `k1_01_erste_fahrt`.
pub fn level_name(id: &str, authored: &str) -> String {
    t_or(&format!("level.{id}.name"), authored)
}

/// Localized station label (fallback = authored `sink.label`). Unknown labels
/// (e.g. dynamic `Z{n}` sandbox labels) fall back cleanly to the raw value.
pub fn station_label(authored: &str) -> String {
    t_or(&format!("station.{authored}"), authored)
}

/// Display label for a source: its custom name, or `Q{id}` when unnamed.
/// Sources gained an optional label after launch; an empty one (old level
/// files, freshly placed sources) renders as the stable `Q{id}` fallback.
pub fn source_label(id: u32, label: &str) -> String {
    if label.is_empty() {
        format!("Q{id}")
    } else {
        station_label(label)
    }
}

/// Display label for a sink: its custom name, or `Z{id}` when unnamed (a
/// hand-edited level file with a blank label — editor-placed sinks default to
/// `Z{id}`). Mirrors [`source_label`].
pub fn sink_label(id: u32, label: &str) -> String {
    if label.is_empty() {
        format!("Z{id}")
    } else {
        station_label(label)
    }
}

/// Localized level briefing (fallback = authored `LevelMeta.briefing`). `id`
/// is the level file stem; the empty sandbox briefing stays empty.
pub fn briefing(id: &str, authored: &str) -> String {
    if authored.is_empty() {
        return String::new();
    }
    t_or(&format!("level.{id}.briefing"), authored)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_or_falls_back_to_authored() {
        // Without a loaded table (or with a missing key) t_or returns the
        // fallback, NOT the key.
        let got = t_or("level.unbekannt.name", "1.1 Erste Fahrt");
        assert_eq!(got, "1.1 Erste Fahrt");
        assert_ne!(got, "level.unbekannt.name");
    }

    /// The real localization checker: every dynamic string the UI can build
    /// at runtime must have its key in BOTH language tables. The existing
    /// `language_tables_cover_identical_keys` only proves the two tables agree
    /// with each other — never that the CODE routes a string through `t()`.
    /// Reads the tables directly (not via `t`) so a key present only in the
    /// German fallback cannot mask a missing English entry.
    #[test]
    fn dynamic_keys_present_in_both_tables() {
        use crate::state::Tool;
        use crate::ui::edit_hud::tool_key;
        use crate::ui::encyclopedia::TOOL_HELP_KEYS;
        use crate::ui::pause::PAUSE_KEYS;
        use crate::ui::select::{DECODE_ERROR_KEYS, SELECT_CHAPTER_KEYS};
        use crate::ui::valerr::{BUILD_ISSUE_KEYS, VALERR_KEYS};

        fn table(lang: &str) -> BTreeMap<String, String> {
            let path = format!("{}/assets/i18n/{lang}.ron", env!("CARGO_MANIFEST_DIR"));
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
            ron::from_str(&text).unwrap_or_else(|e| panic!("parse {path}: {e}"))
        }
        let de = table("de");
        let en = table("en");

        let mut keys: Vec<String> = [
            "common.train",
            "common.sink",
            "edit.tool_label",
            "edit.more_errors",
            "run.train_due",
            "run.train_waiting",
            "result.export_failed",
            "select.import_ok",
            "select.import_unknown",
            "select.import_sandbox",
            "console.export_ok",
            "console.export_failed",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        keys.extend(VALERR_KEYS.iter().map(|k| k.to_string()));
        keys.extend(BUILD_ISSUE_KEYS.iter().map(|k| k.to_string()));
        keys.extend(DECODE_ERROR_KEYS.iter().map(|k| k.to_string()));
        keys.extend(SELECT_CHAPTER_KEYS.iter().map(|k| k.to_string()));
        keys.extend(TOOL_HELP_KEYS.iter().map(|k| k.to_string()));
        keys.extend(PAUSE_KEYS.iter().map(|k| k.to_string()));
        keys.extend(Tool::ALL.iter().map(|&tool| tool_key(tool).to_string()));

        for key in keys {
            assert!(de.contains_key(&key), "de.ron fehlt dynamischer Key: {key}");
            assert!(en.contains_key(&key), "en.ron fehlt dynamischer Key: {key}");
        }
    }
}
