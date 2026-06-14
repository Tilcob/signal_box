//! Dev authoring tool (#4 of optimierung/07): fills MISSING `level.*` /
//! `station.*` i18n keys for every level under `assets/levels/`, in BOTH
//! tables, with the authored value as a placeholder. English placeholders get
//! a `⟨TODO⟩` sentinel so untranslated entries stay findable. Never overwrites
//! an existing key, so real translations are safe; never reorders the file
//! (appends before the closing brace), so diffs stay small.
//!
//! Run from the repo root: `cargo run --bin i18n_fill`.

use std::collections::BTreeMap;
use stellwerk_sim::level::LevelDef;

/// Marks an untranslated English placeholder. A separate report lists these.
/// ASCII only — the game font has no fancy bracket glyphs.
const TODO: &str = "[TODO] ";

fn main() {
    let root = env!("CARGO_MANIFEST_DIR");
    let levels_dir = format!("{root}/assets/levels");

    // key -> authored (German) value, the natural fallback.
    let mut expected: BTreeMap<String, String> = BTreeMap::new();
    let read_dir = std::fs::read_dir(&levels_dir).expect("assets/levels");
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("ron") {
            continue;
        }
        let id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let text = std::fs::read_to_string(&path).expect("read level");
        let def: LevelDef = match ron::from_str(&text) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("skip {path:?}: {e}");
                continue;
            }
        };
        expected.insert(format!("level.{id}.name"), def.sim.name.clone());
        if !def.meta.briefing.is_empty() {
            expected.insert(format!("level.{id}.briefing"), def.meta.briefing.clone());
        }
        for sink in &def.sim.sinks {
            // Dynamic sandbox labels (Z{n}) fall back to the raw value.
            if sink.label.starts_with('Z') && sink.label[1..].chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            expected.insert(format!("station.{}", sink.label), sink.label.clone());
        }
    }

    let de_added = fill_table(&format!("{root}/assets/i18n/de.ron"), &expected, false);
    let en_added = fill_table(&format!("{root}/assets/i18n/en.ron"), &expected, true);
    println!("i18n_fill: de +{de_added}, en +{en_added} key(s).");
    if en_added > 0 {
        println!("  English entries marked with {TODO:?} need translating.");
    }

    report_untranslated(&format!("{root}/assets/i18n/en.ron"));
}

/// Inserts every expected key not already present, preserving file order
/// (appends before the final `}`). German gets the authored value; English the
/// `⟨TODO⟩`-marked placeholder. Returns how many keys were added.
fn fill_table(path: &str, expected: &BTreeMap<String, String>, placeholder: bool) -> usize {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let existing: BTreeMap<String, String> =
        ron::from_str(&text).unwrap_or_else(|e| panic!("parse {path}: {e}"));

    let mut additions = String::new();
    let mut count = 0;
    for (key, authored) in expected {
        if existing.contains_key(key) {
            continue;
        }
        let value = if placeholder {
            format!("{TODO}{authored}")
        } else {
            authored.clone()
        };
        // {:?} emits a quoted, escaped string — valid RON, Unicode preserved.
        additions.push_str(&format!("    {key:?}: {value:?},\n"));
        count += 1;
    }
    if count == 0 {
        return 0;
    }
    let idx = text.rfind('}').expect("table has a closing brace");
    let mut out = String::with_capacity(text.len() + additions.len());
    out.push_str(&text[..idx]);
    out.push_str(&additions);
    out.push_str(&text[idx..]);
    std::fs::write(path, out).unwrap_or_else(|e| panic!("write {path}: {e}"));
    count
}

/// Lists every still-untranslated English key (value carries the sentinel).
fn report_untranslated(en_path: &str) {
    let text = std::fs::read_to_string(en_path).expect("read en");
    let en: BTreeMap<String, String> = ron::from_str(&text).expect("parse en");
    let open: Vec<&String> = en
        .iter()
        .filter(|(_, v)| v.starts_with(TODO))
        .map(|(k, _)| k)
        .collect();
    if open.is_empty() {
        println!("All English keys translated.");
    } else {
        println!("Untranslated English keys ({}):", open.len());
        for k in open {
            println!("  {k}");
        }
    }
}
