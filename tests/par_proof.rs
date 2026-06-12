//! Par hardening (M2 plan §2.3): every shipped level carries at least one
//! designer solution in `assets/levels/solutions/`; CI proves headlessly
//! that (a) each solution succeeds and (b) every par axis is reached by
//! some stored solution. Unreachable pars are thereby technically
//! impossible — GDD §7.7's promise.
//!
//! Solution files: `solutions/<level_id>.ron` plus optional variants
//! `solutions/<level_id>__<name>.ron` (e.g. a separate material-optimal
//! build). Bless flow: on a par miss the test prints the best achieved
//! values — adjust the level's par (or improve the solution) deliberately.

use stellwerk_sim::layout::Layout;
use stellwerk_sim::level::Level;
use stellwerk_sim::units::Tick;
use stellwerk_sim::{Outcome, Sim};

fn read_ron<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> T {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
    ron::from_str(&text).unwrap_or_else(|e| panic!("parse {path:?}: {e}"))
}

#[test]
fn every_level_par_is_proven() {
    let root = env!("CARGO_MANIFEST_DIR");
    let levels_dir = format!("{root}/assets/levels");
    let solutions_dir = format!("{root}/assets/levels/solutions");

    let mut level_files: Vec<_> = std::fs::read_dir(&levels_dir)
        .expect("levels dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("ron"))
        .collect();
    level_files.sort();

    let mut failures: Vec<String> = Vec::new();
    for level_path in level_files {
        let id = level_path
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let level: Level = read_ron(&level_path);

        let mut solution_files: Vec<_> = std::fs::read_dir(&solutions_dir)
            .expect("solutions dir")
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                stem == id || stem.starts_with(&format!("{id}__"))
            })
            .collect();
        solution_files.sort();
        if solution_files.is_empty() {
            failures.push(format!("{id}: keine Designer-Lösung hinterlegt"));
            continue;
        }

        let mut best_throughput = u64::MAX;
        let mut best_material = u32::MAX;
        let mut best_lateness = u64::MAX;
        for solution_path in &solution_files {
            let layout: Layout = read_ron(solution_path);
            let errors = stellwerk_sim::validate(&level, &layout);
            if !errors.is_empty() {
                failures.push(format!("{solution_path:?} validiert nicht: {errors:?}"));
                continue;
            }
            let mut sim = Sim::new(&level, &layout).expect("validated");
            match sim.run(Tick(50_000)) {
                Outcome::Success { score } => {
                    best_throughput = best_throughput.min(score.throughput.0);
                    best_material = best_material.min(score.material);
                    best_lateness = best_lateness.min(score.lateness);
                }
                other => failures.push(format!("{solution_path:?} scheitert: {other:?}")),
            }
        }

        // Bless aid: visible with `-- --nocapture`.
        eprintln!(
            "{id}: erreicht throughput {best_throughput}, material {best_material}, lateness {best_lateness}"
        );
        let par = &level.par;
        if best_throughput > par.throughput.0
            || best_material > par.material
            || best_lateness > par.lateness
        {
            failures.push(format!(
                "{id}: Par nicht bewiesen — erreicht (best je Achse): throughput {best_throughput}, \
                 material {best_material}, lateness {best_lateness}; Par: {}, {}, {}",
                par.throughput.0, par.material, par.lateness
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "Par-Beweis fehlgeschlagen:\n{}",
        failures.join("\n")
    );
}
