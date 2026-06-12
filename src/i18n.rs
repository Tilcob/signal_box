//! i18n (M2): all UI strings come from RON tables under `assets/i18n/`
//! (`de.ron`, `en.ron`). [`t`] looks up the active language with fallback
//! chain: active table → German table → the raw key (visible programmer
//! error, never a panic). The language toggle lives in the level select;
//! the choice persists in the progress file.

use std::collections::BTreeMap;
use std::sync::RwLock;

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
    let table = Table {
        active: load_table(lang),
        fallback: load_table("de"),
    };
    *TABLE.write().expect("i18n lock") = Some(table);
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
