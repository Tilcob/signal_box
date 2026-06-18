//! Dev authoring tool: the "bless" flow. Runs every
//! stored designer solution through the headless sim — exactly like
//! `tests/par_proof.rs` — and reports the best value reached per axis. With
//! `--write` it rewrites the `par:` line of each level file in place, replacing
//! only that block so the hand-authored comments and layout survive.
//!
//! Run from the repo root:
//!   `cargo run --bin par_suggest`            (dry-run: print suggestions)
//!   `cargo run --bin par_suggest -- --write` (apply to the level files)
//!   `cargo run --bin par_suggest -- <id>`    (one level only)

use stellwerk_sim::layout::Layout;
use stellwerk_sim::level::LevelDef;
use stellwerk_sim::units::Tick;
use stellwerk_sim::{Outcome, Sim};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let write = args.iter().any(|a| a == "--write");
    let only = args.iter().find(|a| !a.starts_with("--")).cloned();

    let root = env!("CARGO_MANIFEST_DIR");
    let levels_dir = format!("{root}/assets/levels");
    let solutions_dir = format!("{levels_dir}/solutions");

    let mut files: Vec<_> = std::fs::read_dir(&levels_dir)
        .expect("assets/levels")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("ron"))
        .collect();
    files.sort();

    for path in files {
        let id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        if let Some(only) = &only
            && only != &id
        {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read level");
        let def: LevelDef = match ron::from_str(&text) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("skip {id}: {e}");
                continue;
            }
        };

        let mut solutions: Vec<_> = std::fs::read_dir(&solutions_dir)
            .expect("solutions dir")
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                stem == id || stem.starts_with(&format!("{id}__"))
            })
            .collect();
        solutions.sort();
        if solutions.is_empty() {
            eprintln!("{id}: keine Designer-Lösung — übersprungen");
            continue;
        }

        let (mut bt, mut bm, mut bl) = (u64::MAX, u32::MAX, u64::MAX);
        let mut ok = true;
        for sp in &solutions {
            let layout: Layout = match ron::from_str(&std::fs::read_to_string(sp).expect("read sol")) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("{sp:?}: {e}");
                    ok = false;
                    break;
                }
            };
            if !stellwerk_sim::validate(&def.sim, &layout).is_empty() {
                eprintln!("{sp:?}: validiert nicht");
                ok = false;
                break;
            }
            match Sim::new(&def.sim, &layout).expect("validated").run(Tick(50_000)) {
                Outcome::Success { score } => {
                    bt = bt.min(score.throughput.0);
                    bm = bm.min(score.material);
                    bl = bl.min(score.lateness);
                }
                other => {
                    eprintln!("{sp:?}: {other:?}");
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            continue;
        }

        let par = &def.sim.par;
        println!(
            "{id}: par (throughput: {bt}, material: {bm}, lateness: {bl})   [aktuell: {}, {}, {}]",
            par.throughput.0, par.material, par.lateness
        );

        if write {
            // Match the file's RON dialect: hand-authored files enable
            // `unwrap_newtypes` and write the bare `throughput: 60`, machine-
            // written files (no header) wrap the Tick newtype as `(60)`. Only
            // throughput is a newtype; material/lateness are plain ints in both.
            // ponytail: header presence is the dialect tell — heuristic, fine
            // for a dev tool. Drop it once one dialect wins repo-wide.
            let throughput = if text.contains("unwrap_newtypes") {
                format!("{bt}")
            } else {
                format!("({bt})")
            };
            let new_par = format!("par: (throughput: {throughput}, material: {bm}, lateness: {bl})");
            match replace_par(&text, &new_par) {
                Some(updated) => {
                    std::fs::write(&path, updated).expect("write level");
                    println!("  → geschrieben");
                }
                None => eprintln!("  ! par-Block in {path:?} nicht gefunden"),
            }
        }
    }

    if !write {
        println!("\n(dry-run — mit `-- --write` in die Level-Dateien zurückschreiben)");
    }
}

/// Replaces the single `par: ( … )` block, balancing parens to find its end.
/// A machine-written file (no `unwrap_newtypes` header) wraps the throughput
/// newtype as `(0)`, so the FIRST `)` closes that inner value, not the block —
/// taking it would slice the par block in half and emit invalid RON. Everything
/// else (comments, formatting) is preserved verbatim. The search is anchored
/// after `sim:`: par lives inside `sim`, so a briefing in the earlier `meta`
/// block that happens to contain "par:" cannot be matched first.
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

    /// The bug that ate k1_06_test: a headerless file wraps throughput as `(0)`,
    /// so a first-`)` scan sliced the block. The whole multi-line block (inner
    /// parens and all) must be replaced, leaving balanced, re-parseable RON.
    #[test]
    fn replaces_block_with_nested_parens() {
        let text = "(\n  sim: (\n    par: (\n      throughput: (0),\n      material: 0,\n      lateness: 0,\n    ),\n  ),\n)\n";
        let out = replace_par(text, "par: (throughput: (276), material: 34, lateness: 430)").unwrap();
        assert!(out.contains("par: (throughput: (276), material: 34, lateness: 430)"));
        assert!(!out.contains("throughput: (0)"), "old block must be gone");
        assert!(!out.contains("material: 0"), "orphaned tail must be gone");
        assert_eq!(out.matches('(').count(), out.matches(')').count(), "balanced");
    }
}
