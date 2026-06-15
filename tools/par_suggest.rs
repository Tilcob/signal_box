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
            let new_par = format!("par: (throughput: {bt}, material: {bm}, lateness: {bl})");
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

/// Replaces the single `par: ( … )` block — its contents have no nested
/// parens, so the first `)` after `par:` closes it. Everything else (comments,
/// formatting) is preserved verbatim. The search is anchored after `sim:`: par
/// lives inside `sim`, so a briefing in the earlier `meta` block that happens
/// to contain "par:" cannot be matched first.
fn replace_par(text: &str, new_par: &str) -> Option<String> {
    let sim = text.find("sim:")?;
    let start = sim + text[sim..].find("par:")?;
    let open = text[start..].find('(')? + start;
    let close = text[open..].find(')')? + open;
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..start]);
    out.push_str(new_par);
    out.push_str(&text[close + 1..]);
    Some(out)
}
