//! Dev-only authoring helpers (feature `dev`). They write
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
use stellwerk_sim::units::Tick;
use stellwerk_sim::{Outcome, Sim};

/// Marks an untranslated English placeholder (shared with the `i18n_fill` CLI).
/// ASCII only — the game font has no fancy bracket glyphs.
const TODO: &str = "[TODO] ";

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

/// The solution file stems on disk that belong to `id`: the primary `<id>` and
/// every `<id>__variant`, sorted. Same match rule as [`delete_level`] and the
/// `par_proof`/`par_suggest` tools. For showing the dev what exists after a
/// save, so an overwrite is never a silent surprise. Empty if the dir is gone.
pub fn list_solutions(id: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(solutions_dir()) {
        for e in rd.flatten() {
            if let Some(stem) = e.path().file_stem().and_then(|s| s.to_str())
                && (stem == id || stem.starts_with(&format!("{id}__")))
            {
                out.push(stem.to_string());
            }
        }
    }
    out.sort();
    out
}

/// The "bless" step inlined for the in-game solution-save (the `par_suggest`
/// CLI does the same): runs every stored solution of `id` through the headless
/// sim, takes the best value per axis, and rewrites the level file's `par:`
/// block in place. Returns a status line, or an error when a solution is
/// missing / invalid / not `Success` — in which case the par is left untouched,
/// so an unprovable par is never written.
pub fn suggest_and_write_par(id: &str) -> Result<String, String> {
    let level_path = levels_dir().join(format!("{id}.ron"));
    let text = std::fs::read_to_string(&level_path).map_err(|e| e.to_string())?;
    let def: LevelDef = ron::from_str(&text).map_err(|e| e.to_string())?;

    // Solution files belonging to `id` — same match rule as `list_solutions`.
    let mut solutions: Vec<PathBuf> = std::fs::read_dir(solutions_dir())
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            stem == id || stem.starts_with(&format!("{id}__"))
        })
        .collect();
    solutions.sort();
    if solutions.is_empty() {
        return Err(format!("{id}: keine Lösung gefunden"));
    }

    let (mut bt, mut bm, mut bl) = (u64::MAX, u32::MAX, u64::MAX);
    for sp in &solutions {
        let stext = std::fs::read_to_string(sp).map_err(|e| e.to_string())?;
        let layout: Layout = ron::from_str(&stext).map_err(|e| format!("{}: {e}", sp.display()))?;
        if !stellwerk_sim::validate(&def.sim, &layout).is_empty() {
            return Err(format!("{}: validiert nicht", sp.display()));
        }
        let mut sim = Sim::new(&def.sim, &layout).map_err(|_| format!("{}: Sim-Aufbau", sp.display()))?;
        match sim.run(Tick(50_000)) {
            Outcome::Success { score } => {
                bt = bt.min(score.throughput.0);
                bm = bm.min(score.material);
                bl = bl.min(score.lateness);
            }
            other => return Err(format!("{}: {other:?}", sp.display())),
        }
    }

    // Match the file's RON dialect (see `par_suggest::replace_par`): only
    // throughput is a newtype — bare with the `unwrap_newtypes` header, wrapped
    // without it; material/lateness are plain ints in both.
    let throughput = if text.contains("unwrap_newtypes") {
        format!("{bt}")
    } else {
        format!("({bt})")
    };
    let new_par = format!("par: (throughput: {throughput}, material: {bm}, lateness: {bl})");
    let updated = replace_par(&text, &new_par)
        .ok_or_else(|| format!("par-Block in {} nicht gefunden", level_path.display()))?;
    std::fs::write(&level_path, updated).map_err(|e| e.to_string())?;
    Ok(format!("Par gesetzt: throughput {bt}, material {bm}, lateness {bl}"))
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
    // Named sources localize through the same `station.<label>` key as sinks
    // (see `i18n::source_label`); unnamed ones render as `Q{id}`.
    for source in &sim.sources {
        if source.label.is_empty() {
            continue;
        }
        pairs.push((format!("station.{}", source.label), source.label.clone()));
    }
    let def = LevelDef { meta, sim };
    let path = levels_dir().join(format!("{id}.ron"));
    std::fs::write(&path, ron_pretty(&def)?).map_err(|e| e.to_string())?;
    append_missing(&i18n_path("de"), &pairs, false);
    append_missing(&i18n_path("en"), &pairs, true);
    Ok(path)
}

/// Delete a campaign level COMPLETELY: its file, every solution variant, and
/// its level-specific i18n keys (`level.<id>.*`). `station.<label>` keys are
/// deliberately left — labels like `OST` are shared across levels, so removing
/// one level's must not drop a station another level still uses. Best-effort
/// (dev tool): missing pieces are skipped silently.
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
/// order). German = authored value; English = `[TODO]`-marked placeholder.
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
/// our tables. `split_inclusive` keeps each line's terminator, so the file's
/// existing newline style (LF/CRLF) and trailing structure survive untouched —
/// no spurious whole-file diffs. Keeps both tables in lock-step so the i18n
/// parity test stays green.
fn remove_lines_for_keys(path: &PathBuf, keys: &[String]) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let out: String = text
        .split_inclusive('\n')
        .filter(|line| {
            let trimmed = line.trim_start();
            !keys.iter().any(|k| trimmed.starts_with(&format!("{k:?}:")))
        })
        .collect();
    let _ = std::fs::write(path, out);
}

/// Replaces the single `par: ( … )` block, balancing parens to find its end so a
/// machine-written `throughput: (0)` (whose first `)` closes the inner newtype,
/// not the block) is handled. Everything else — comments, formatting — survives
/// verbatim. Anchored after `sim:` so a briefing in `meta` containing "par:"
/// can't match first. Mirrors `par_suggest::replace_par`.
fn replace_par(text: &str, new_par: &str) -> Option<String> {
    let sim = text.find("sim:")?;
    let start = sim + text[sim..].find("par:")?;
    let open = text[start..].find('(')? + start;
    let mut depth = 0usize;
    let mut close = None;
    for (i, c) in text[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(open + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..start]);
    out.push_str(new_par);
    out.push_str(&text[close + 1..]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::replace_par;

    /// A headerless file wraps throughput as `(0)`, so a first-`)` scan would
    /// slice the block. The whole nested block must be replaced, leaving
    /// balanced, re-parseable RON.
    #[test]
    fn replaces_par_block_with_nested_parens() {
        let text = "(\n  sim: (\n    par: (\n      throughput: (0),\n      material: 0,\n      lateness: 0,\n    ),\n  ),\n)\n";
        let out = replace_par(text, "par: (throughput: (276), material: 34, lateness: 430)").unwrap();
        assert!(out.contains("par: (throughput: (276), material: 34, lateness: 430)"));
        assert!(!out.contains("throughput: (0)"), "old block must be gone");
        assert_eq!(out.matches('(').count(), out.matches(')').count(), "balanced");
    }
}
