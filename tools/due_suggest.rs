//! Dev authoring tool: derive each train's `due` from the canonical designer
//! solution's actual arrival plus a slack budget, so that 0 lateness is
//! achievable and the "punctuality" axis measures the timetable rather than the
//! distance to the designer's solution. Sibling of `par_suggest` — run that
//! afterwards to refresh `par.lateness` (it drops to 0 once `due` is blessed,
//! because the reference solution meets every deadline by construction).
//!
//! Slack is a uniform percentage of each train's run time (arrival − depart),
//! so a long cross-map run tolerates proportionally more delay than a short
//! hop. The reference is the BASE solution (`{id}.ron`), never the
//! axis-specialised `{id}__*` variants — those optimise one score axis and
//! carry no canonical timetable.
//!
//! Run from the repo root:
//!   `cargo run --bin due_suggest`               (dry-run, 10% slack)
//!   `cargo run --bin due_suggest -- --slack 15` (15% slack)
//!   `cargo run --bin due_suggest -- --write`    (apply to the level files)
//!   `cargo run --bin due_suggest -- <id>`       (one level only)

use stellwerk_sim::layout::Layout;
use stellwerk_sim::level::LevelDef;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut write = false;
    let mut slack_pct: u64 = 10;
    let mut only: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--write" => write = true,
            "--slack" => slack_pct = it.next().and_then(|v| v.parse().ok()).unwrap_or(slack_pct),
            s if s.starts_with("--") => eprintln!("unbekannte Flag: {s}"),
            s => only = Some(s.to_string()),
        }
    }

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

        // The canonical timetable comes from the base solution only.
        let sol_path = format!("{solutions_dir}/{id}.ron");
        let layout: Layout = match std::fs::read_to_string(&sol_path) {
            Ok(t) => match ron::from_str(&t) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("{id}: Basis-Lösung unlesbar: {e}");
                    continue;
                }
            },
            Err(_) => {
                eprintln!("{id}: keine Basis-Lösung ({id}.ron) — übersprungen");
                continue;
            }
        };
        if !stellwerk_sim::validate(&def.sim, &layout).is_empty() {
            eprintln!("{id}: Basis-Lösung validiert nicht — übersprungen");
            continue;
        }

        // The slack formula lives once in the sim core (shared with the in-game
        // dev/sandbox calibration); this tool only prints + writes the result.
        let dues = match stellwerk_sim::suggest_dues(&def.sim, &layout, slack_pct) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{id}: {e}");
                continue;
            }
        };
        println!("{id}: slack {slack_pct}%");
        for (entry, due) in def.sim.schedule.iter().zip(&dues) {
            println!("  Zug {}: due {} → {}", entry.train.0, entry.due.0, due.0);
        }
        let new_dues: Vec<u64> = dues.iter().map(|t| t.0).collect();

        if write {
            match rewrite_schedule_dues(&text, &new_dues) {
                Some(updated) => {
                    std::fs::write(&path, updated).expect("write level");
                    println!("  → geschrieben");
                }
                None => eprintln!("  ! schedule/due in {path:?} nicht sauber gefunden"),
            }
        }
    }

    if !write {
        println!("\n(dry-run — mit `-- --write` in die Level-Dateien zurückschreiben,");
        println!(" danach `cargo run --bin par_suggest -- --write` für par.lateness)");
    }
}

/// Replaces the value of every `due:` field inside the `schedule:` list, in
/// order, with `new_dues`. Each token's form is preserved per-token: a wrapped
/// newtype `(80)` stays wrapped, a bare `80` (unwrap_newtypes dialect) stays
/// bare — so the file's RON dialect, comments and layout survive untouched.
/// The search is bounded to the schedule list span (anchored after `sim:` →
/// `schedule:` → its matching `]`), so a stray "due" elsewhere can't match.
/// `None` if the span isn't found or the `due:` count doesn't match the
/// schedule length (a guard against a malformed splice).
fn rewrite_schedule_dues(text: &str, new_dues: &[u64]) -> Option<String> {
    let sim = text.find("sim:")?;
    let sched = sim + text[sim..].find("schedule:")?;
    let open = sched + text[sched..].find('[')?;
    // Schedule entries nest only parens, never brackets, so depth on `[`/`]`
    // alone locates the list's matching close.
    let mut depth = 0usize;
    let mut close = None;
    for (i, c) in text[open..].char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
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

    // (value_start, value_end, was_parenthesised) for each `due:` in order.
    let mut ranges: Vec<(usize, usize, bool)> = Vec::new();
    let mut cursor = open;
    while let Some(rel) = text[cursor..close].find("due:") {
        let after = cursor + rel + "due:".len();
        let ws = text[after..close].find(|c: char| !c.is_whitespace())?;
        let val_start = after + ws;
        if text.as_bytes()[val_start] == b'(' {
            let mut d = 0usize;
            let mut end = None;
            for (i, c) in text[val_start..close].char_indices() {
                match c {
                    '(' => d += 1,
                    ')' => {
                        d -= 1;
                        if d == 0 {
                            end = Some(val_start + i + 1);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            ranges.push((val_start, end?, true));
        } else {
            let end = val_start + text[val_start..close].find(|c: char| !c.is_ascii_digit())?;
            ranges.push((val_start, end, false));
        }
        cursor = ranges.last().expect("just pushed").1;
    }
    if ranges.len() != new_dues.len() {
        return None;
    }

    let mut out = String::with_capacity(text.len());
    let mut prev = 0;
    for (&(s, e, parens), &d) in ranges.iter().zip(new_dues) {
        out.push_str(&text[prev..s]);
        if parens {
            out.push_str(&format!("({d})"));
        } else {
            out.push_str(&format!("{d}"));
        }
        prev = e;
    }
    out.push_str(&text[prev..]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::rewrite_schedule_dues;

    /// Rewrites every entry's due in order, preserving the wrapped-newtype
    /// dialect, the depart values, and surrounding text (a comment survives).
    #[test]
    fn rewrites_each_due_in_order_wrapped() {
        let text = "(\n  sim: (\n    schedule: [\n      ( train: (0), depart: (20), due: (80) ),\n      // second\n      ( train: (1), depart: (50), due: (90) ),\n    ],\n  ),\n)\n";
        let out = rewrite_schedule_dues(text, &[92, 101]).unwrap();
        assert!(out.contains("depart: (20), due: (92)"), "first due rewritten, depart kept");
        assert!(out.contains("depart: (50), due: (101)"), "second due rewritten");
        assert!(out.contains("// second"), "comment survives");
        assert!(!out.contains("(80)") && !out.contains("(90)"), "old dues gone");
    }

    /// Bare unwrap_newtypes dialect (`due: 80`) stays bare.
    #[test]
    fn rewrites_bare_dialect() {
        let text = "( sim: ( schedule: [ ( train: 0, depart: 20, due: 80 ), ], ), )";
        let out = rewrite_schedule_dues(text, &[92]).unwrap();
        assert!(out.contains("due: 92"), "bare due rewritten");
        assert!(!out.contains("(92)"), "no parens added");
    }

    /// A due-count mismatch refuses rather than splicing a half-updated file.
    #[test]
    fn refuses_on_count_mismatch() {
        let text = "( sim: ( schedule: [ ( due: (80) ), ( due: (90) ), ], ), )";
        assert!(rewrite_schedule_dues(text, &[1]).is_none());
    }
}
