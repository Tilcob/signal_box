//! Localization coverage (M2 plan §3): both language tables must define the
//! identical key set — a missing key would silently fall back and drift.

use std::collections::BTreeSet;

fn keys(lang: &str) -> BTreeSet<String> {
    let path = format!("{}/assets/i18n/{lang}.ron", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let map: std::collections::BTreeMap<String, String> =
        ron::from_str(&text).unwrap_or_else(|e| panic!("parse {path}: {e}"));
    map.into_keys().collect()
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
