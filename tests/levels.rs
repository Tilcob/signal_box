//! Every shipped level must be well-formed on its own: parseable, and valid
//! with an EMPTY player layout (sources/sinks anchored on designer track,
//! schedule consistent). The full par-proof harness (designer solutions in
//! CI) is M2 — this catches hand-authoring mistakes today.

use stellwerk_sim::layout::Layout;
use stellwerk_sim::level::Level;

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

#[test]
fn all_levels_parse_and_validate_empty() {
    let files = level_files();
    assert_eq!(files.len(), 15, "M2 content stand: 15 levels (plan M2 §4)");
    for path in files {
        let text = std::fs::read_to_string(&path).expect("readable");
        let level: Level =
            ron::from_str(&text).unwrap_or_else(|e| panic!("{path:?} does not parse: {e}"));
        let errors = stellwerk_sim::validate(&level, &Layout::default());
        assert!(
            errors.is_empty(),
            "{path:?} invalid with empty player layout: {errors:#?}"
        );
        assert!(!level.schedule.is_empty(), "{path:?} has no trains");
        assert!(level.par.material > 0, "{path:?} has no material par");
    }
}
