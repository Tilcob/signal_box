//! Localization coverage (M2 plan §3): both language tables must define the
//! identical key set — a missing key would silently fall back and drift.

use std::collections::BTreeSet;
use stellwerk_sim::level::Level;

fn keys(lang: &str) -> BTreeSet<String> {
    let path = format!("{}/assets/i18n/{lang}.ron", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let map: std::collections::BTreeMap<String, String> =
        ron::from_str(&text).unwrap_or_else(|e| panic!("parse {path}: {e}"));
    map.into_keys().collect()
}

fn level_files() -> Vec<std::path::PathBuf> {
    let dir = format!("{}/assets/levels", env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .expect("assets/levels exists")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ron"))
        .collect();
    files.sort();
    files
}

fn level_stem(path: &std::path::Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .expect("level file stem")
        .to_string()
}

#[test]
fn language_tables_cover_identical_keys() {
    let de = keys("de");
    let en = keys("en");
    let missing_in_en: Vec<_> = de.difference(&en).collect();
    let missing_in_de: Vec<_> = en.difference(&de).collect();
    assert!(
        missing_in_en.is_empty() && missing_in_de.is_empty(),
        "key drift — fehlt in en: {missing_in_en:?}, fehlt in de: {missing_in_de:?}"
    );
}

#[test]
fn every_level_name_has_a_key_in_both_languages() {
    let de = keys("de");
    let en = keys("en");
    for path in level_files() {
        let key = format!("level.{}.name", level_stem(&path));
        assert!(de.contains(&key), "de.ron fehlt {key}");
        assert!(en.contains(&key), "en.ron fehlt {key}");
    }
}

#[test]
fn every_station_label_has_a_key() {
    let de = keys("de");
    let en = keys("en");
    for path in level_files() {
        let text = std::fs::read_to_string(&path).expect("readable");
        let level: Level =
            ron::from_str(&text).unwrap_or_else(|e| panic!("{path:?} does not parse: {e}"));
        for sink in &level.sinks {
            // Dynamic sandbox labels (Z{n}) fall back to the raw value and
            // need no table key.
            if sink.label.starts_with('Z') && sink.label[1..].chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let key = format!("station.{}", sink.label);
            assert!(de.contains(&key), "de.ron fehlt {key}");
            assert!(en.contains(&key), "en.ron fehlt {key}");
        }
    }
}
