//! Dev-only authoring helpers (feature `dev`, plan optimierung/07). They write
//! into the repo's `assets/` tree and only make sense when the game runs from
//! the working tree — never in a shipped build, hence the whole module is
//! feature-gated.
//!
//! Files are written WITHOUT the `#![enable(unwrap_newtypes, implicit_some)]`
//! RON header: serde-RON emits the wrapped form (`Tick(60)`, `Some(x)`) which
//! parses fine without the header. Hand-authored files keep their header and
//! the unwrapped form; both coexist because the header is per-file.
//!
//! These functions only touch files. Reloading the live i18n table and the
//! level catalog is the caller's job (the UI handler has the language and can
//! rebuild the screen) — see `ui::select`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use stellwerk_sim::layout::Layout;
use stellwerk_sim::level::{Level, LevelDef, LevelMeta};

/// Marks an untranslated English placeholder (shared with the `i18n_fill` CLI).
const TODO: &str = "⟨TODO⟩ ";

fn levels_dir() -> PathBuf {
    PathBuf::from("assets/levels")
}
fn solutions_dir() -> PathBuf {
    levels_dir().join("solutions")
}
fn i18n_path(lang: &str) -> PathBuf {
    PathBuf::from(format!("assets/i18n/{lang}.ron"))
}

fn ron_pretty<T: serde::Serialize>(value: &T) -> Result<String, String> {
    ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())
}

/// Tool 2 — save a successful build as a designer solution. Writes
/// `solutions/<id>[__variant].ron`, exactly the file `tests/par_proof.rs` and
/// the `par_suggest` CLI consume.
pub fn write_solution(id: &str, variant: Option<&str>, layout: &Layout) -> Result<PathBuf, String> {
    let name = match variant {
        Some(v) => format!("{id}__{v}.ron"),
        None => format!("{id}.ron"),
    };
    std::fs::create_dir_all(solutions_dir()).map_err(|e| e.to_string())?;
    let path = solutions_dir().join(name);
    std::fs::write(&path, ron_pretty(layout)?).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Tool 1 — write a sandbox build out as a campaign level
/// (`assets/levels/<id>.ron`) and seed placeholder i18n keys for its name,
/// briefing and station labels in BOTH tables. The caller reloads the live
/// table afterwards.
pub fn write_campaign_level(id: &str, meta: LevelMeta, sim: Level) -> Result<PathBuf, String> {
    let mut pairs: Vec<(String, String)> = vec![(format!("level.{id}.name"), sim.name.clone())];
    if !meta.briefing.is_empty() {
        pairs.push((format!("level.{id}.briefing"), meta.briefing.clone()));
    }
    for sink in &sim.sinks {
        if sink.label.starts_with('Z') && sink.label[1..].chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        pairs.push((format!("station.{}", sink.label), sink.label.clone()));
    }
    let def = LevelDef { meta, sim };
    let path = levels_dir().join(format!("{id}.ron"));
    std::fs::write(&path, ron_pretty(&def)?).map_err(|e| e.to_string())?;
    append_missing(&i18n_path("de"), &pairs, false);
    append_missing(&i18n_path("en"), &pairs, true);
    Ok(path)
}

/// Delete a campaign level COMPLETELY: its file, every solution variant, and
/// its i18n keys — so no orphaned solutions or keys linger. Best-effort (dev
/// tool): missing pieces are skipped silently.
pub fn delete_level(id: &str) {
    let _ = std::fs::remove_file(levels_dir().join(format!("{id}.ron")));
    if let Ok(rd) = std::fs::read_dir(solutions_dir()) {
        for e in rd.flatten() {
            let stem = e
                .path()
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            if stem == id || stem.starts_with(&format!("{id}__")) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let keys = [format!("level.{id}.name"), format!("level.{id}.briefing")];
    remove_lines_for_keys(&i18n_path("de"), &keys);
    remove_lines_for_keys(&i18n_path("en"), &keys);
}

/// Appends keys missing from the table before its closing brace (preserving
/// order). German = authored value; English = `⟨TODO⟩`-marked placeholder.
fn append_missing(path: &PathBuf, pairs: &[(String, String)], placeholder: bool) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(existing) = ron::from_str::<BTreeMap<String, String>>(&text) else {
        return;
    };
    let mut additions = String::new();
    for (key, authored) in pairs {
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
    }
    if additions.is_empty() {
        return;
    }
    if let Some(idx) = text.rfind('}') {
        let mut out = String::with_capacity(text.len() + additions.len());
        out.push_str(&text[..idx]);
        out.push_str(&additions);
        out.push_str(&text[idx..]);
        let _ = std::fs::write(path, out);
    }
}

/// Drops every line whose (trimmed) start is `"<key>":` — one entry per line in
/// our tables. Keeps both tables in lock-step so the i18n parity test stays
/// green.
fn remove_lines_for_keys(path: &PathBuf, keys: &[String]) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let kept: Vec<&str> = text
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !keys.iter().any(|k| trimmed.starts_with(&format!("{k:?}:")))
        })
        .collect();
    let mut out = kept.join("\n");
    out.push('\n');
    let _ = std::fs::write(path, out);
}
