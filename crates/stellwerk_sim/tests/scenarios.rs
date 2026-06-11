//! The 20 exit scenarios of M0 (plan §5). W1 ships the driver plus the
//! fixtures for scenarios 1–2; the `_runs` tests go live in W2 when the sim
//! exists (Aufgabe 2.1 removes the `#[ignore]`).

mod common;

use common::load;
use stellwerk_sim::graph::TrackGraph;

/// Loads a scenario, asserts it validates cleanly and builds a graph.
fn builds_clean(name: &str) -> TrackGraph {
    let scenario = load(name);
    let errors = stellwerk_sim::validate(&scenario.level, &scenario.layout);
    assert!(errors.is_empty(), "{name}: validation errors: {errors:#?}");
    stellwerk_sim::build(&scenario.level, &scenario.layout)
        .expect("graph builds after clean validation")
}

#[test]
fn s01_single_train_straight_builds() {
    let graph = builds_clean("s01_single_train_straight");
    assert_eq!(graph.blocks.count, 1, "no signals → exactly one block");
    assert_eq!(graph.sources.len(), 1);
    assert_eq!(graph.sinks.len(), 1);
}

#[test]
fn s02_curves_and_diagonals_builds() {
    let graph = builds_clean("s02_curves_and_diagonals");
    assert_eq!(graph.blocks.count, 1);
}

#[test]
#[ignore = "W2 (Aufgabe 2.1): Sim fehlt noch — Ignore entfernen, sobald Sim::run existiert"]
fn s01_single_train_straight_runs() {
    let _scenario = load("s01_single_train_straight");
    todo!("Aufgabe 2.1: Sim bauen, run() bis Ende, gegen expect prüfen");
}

#[test]
#[ignore = "W2 (Aufgabe 2.1): Sim fehlt noch — Ignore entfernen, sobald Sim::run existiert"]
fn s02_curves_and_diagonals_runs() {
    let _scenario = load("s02_curves_and_diagonals");
    todo!("Aufgabe 2.1: Sim bauen, run() bis Ende, gegen expect prüfen");
}
