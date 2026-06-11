//! Scenario test driver: loads RON fixtures from `tests/scenarios/`.
//! Format and expectations grow with the milestone — see plan §5.

use serde::Deserialize;
use stellwerk_sim::layout::Layout;
use stellwerk_sim::level::Level;
use stellwerk_sim::units::Tick;

#[derive(Debug, Deserialize)]
pub struct Scenario {
    pub level: Level,
    pub layout: Layout,
    /// Read from W2 on, once the `_runs` tests come alive.
    #[allow(dead_code)]
    pub expect: Expect,
}

/// Expected run outcome. Extended in W2+ once `Outcome` exists
/// (Aufgabe 2.1): `Collision { … }`, `Deadlock { … }`, `Misrouting { … }`.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub enum Expect {
    /// All trains arrive correctly, the last one at or before this tick.
    Success { last_arrival_by: Tick },
}

pub fn load(name: &str) -> Scenario {
    let path = format!("{}/tests/scenarios/{name}.ron", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    ron::from_str(&text).unwrap_or_else(|e| panic!("cannot parse {path}: {e}"))
}
