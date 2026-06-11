//! Scenario test driver: loads RON fixtures from `tests/scenarios/` and
//! checks outcomes against expectations (plan §5).

use serde::Deserialize;
use stellwerk_sim::Sim;
use stellwerk_sim::layout::Layout;
use stellwerk_sim::level::Level;
use stellwerk_sim::sim::Outcome;
use stellwerk_sim::units::Tick;

#[derive(Debug, Deserialize)]
pub struct Scenario {
    pub level: Level,
    pub layout: Layout,
    pub expect: Expect,
}

/// Expected run outcome of a scenario.
#[derive(Debug, Deserialize)]
pub enum Expect {
    Success {
        /// Upper bound for the last arrival tick.
        last_arrival_by: u64,
        /// Lower bound — proves a train actually waited somewhere.
        #[serde(default)]
        last_arrival_at_least: Option<u64>,
        /// Exact axis values (s19 verifies the whole scoring).
        #[serde(default)]
        throughput: Option<u64>,
        #[serde(default)]
        material: Option<u32>,
        #[serde(default)]
        lateness: Option<u64>,
    },
    Collision {
        trains: (u32, u32),
    },
    Misrouting {
        train: u32,
    },
    Deadlock {
        cycle: Vec<u32>,
    },
    Stalled {
        waiting: Vec<u32>,
    },
    /// Checked via `check_reachability`, without running the sim.
    Unreachable {
        trains: Vec<u32>,
    },
}

pub fn load(name: &str) -> Scenario {
    let path = format!("{}/tests/scenarios/{name}.ron", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    ron::from_str(&text).unwrap_or_else(|e| panic!("cannot parse {path}: {e}"))
}

pub const MAX_TICKS: Tick = Tick(10_000);

/// Runs a scenario and checks its expectation, with speaking failures.
pub fn run_and_check(name: &str) -> Sim {
    let scenario = load(name);

    if let Expect::Unreachable { trains } = &scenario.expect {
        let unreachable = stellwerk_sim::check_reachability(&scenario.level, &scenario.layout)
            .unwrap_or_else(|e| panic!("{name}: validation errors: {e:#?}"));
        let got: Vec<u32> = unreachable.iter().map(|u| u.train.0).collect();
        assert_eq!(&got, trains, "{name}: unreachable trains differ");
        // Still build a sim so callers can hash etc. — but don't run it.
        return Sim::new(&scenario.level, &scenario.layout).expect("validates");
    }

    let mut sim = Sim::new(&scenario.level, &scenario.layout)
        .unwrap_or_else(|e| panic!("{name}: validation errors: {e:#?}"));
    let outcome = sim.run(MAX_TICKS);

    match (&scenario.expect, &outcome) {
        (
            Expect::Success {
                last_arrival_by,
                last_arrival_at_least,
                throughput,
                material,
                lateness,
            },
            Outcome::Success { score },
        ) => {
            assert!(
                score.throughput.0 <= *last_arrival_by,
                "{name}: last arrival at tick {}, expected by {last_arrival_by}",
                score.throughput.0
            );
            if let Some(at_least) = last_arrival_at_least {
                assert!(
                    score.throughput.0 >= *at_least,
                    "{name}: last arrival at tick {} — nobody waited? expected ≥ {at_least}",
                    score.throughput.0
                );
            }
            if let Some(expected) = throughput {
                assert_eq!(score.throughput.0, *expected, "{name}: throughput");
            }
            if let Some(expected) = material {
                assert_eq!(score.material, *expected, "{name}: material");
            }
            if let Some(expected) = lateness {
                assert_eq!(score.lateness, *expected, "{name}: lateness");
            }
        }
        (Expect::Collision { trains }, Outcome::Collision { trains: got, .. }) => {
            assert_eq!(
                (got.0.0, got.1.0),
                *trains,
                "{name}: collision pair differs"
            );
        }
        (Expect::Misrouting { train }, Outcome::Misrouting { train: got, .. }) => {
            assert_eq!(got.0, *train, "{name}: misrouted train differs");
        }
        (Expect::Deadlock { cycle }, Outcome::Deadlock { cycle: got }) => {
            let got: Vec<u32> = got.iter().map(|t| t.0).collect();
            assert_eq!(&got, cycle, "{name}: deadlock cycle differs");
        }
        (Expect::Stalled { waiting }, Outcome::Stalled { waiting: got }) => {
            let got: Vec<u32> = got.iter().map(|t| t.0).collect();
            assert_eq!(&got, waiting, "{name}: waiting trains differ");
        }
        (expect, outcome) => panic!(
            "{name}: expected {expect:?}, got {outcome:?} at tick {}",
            sim.now().0
        ),
    }
    sim
}
