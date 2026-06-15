//! The 20 exit scenarios plus the golden replay hashes.
//!
//! Golden values: when a scenario first turns green its final hash is
//! "blessed" into `GOLD` below. Any behavior change shows up as a diff —
//! intentional changes update the value in the same commit, with reasoning
//! in the commit message. CI checks the same table on Windows AND Linux.

mod common;

use common::{MAX_TICKS, load, run_and_check};
use stellwerk_sim::Sim;
use stellwerk_sim::graph::TrackGraph;

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
fn s01_single_train_straight_runs() {
    run_and_check("s01_single_train_straight");
}

#[test]
fn s02_curves_and_diagonals_runs() {
    run_and_check("s02_curves_and_diagonals");
}

#[test]
fn s03_two_trains_block_signal() {
    run_and_check("s03_two_trains_block_signal");
}

#[test]
fn s04_rear_end_no_signal() {
    run_and_check("s04_rear_end_no_signal");
}

#[test]
fn s05_head_on_single_track() {
    run_and_check("s05_head_on_single_track");
}

#[test]
fn s06_passing_loop() {
    run_and_check("s06_passing_loop");
}

#[test]
fn s07_switch_default() {
    run_and_check("s07_switch_default");
}

#[test]
fn s08_switch_dest_rule() {
    run_and_check("s08_switch_dest_rule");
}

#[test]
fn s09_switch_rule_order() {
    run_and_check("s09_switch_rule_order");
}

#[test]
fn s10_misrouting_wrong_sink() {
    run_and_check("s10_misrouting_wrong_sink");
}

#[test]
fn s11_misrouting_dead_end() {
    run_and_check("s11_misrouting_dead_end");
}

#[test]
fn s12_reachability_check() {
    run_and_check("s12_reachability_check");
}

#[test]
fn s13_block_only_crossing_deadlocks() {
    run_and_check("s13_block_only_crossing");
}

#[test]
fn s14_chain_signal_crossing() {
    run_and_check("s14_chain_signal_crossing");
}

#[test]
fn s15_chain_reservation_timing() {
    run_and_check("s15_chain_reservation_timing");
}

#[test]
fn s16_source_fifo() {
    run_and_check("s16_source_fifo");
}

#[test]
fn s17_long_train_two_blocks() {
    run_and_check("s17_long_train_two_blocks");
}

#[test]
fn s18_ring_self_jam_stalls() {
    run_and_check("s18_ring_self_jam");
}

#[test]
fn s19_full_scoring() {
    run_and_check("s19_full_scoring");
}

/// Scenario 20: determinism. Two fresh runs of s14 produce the
/// identical per-tick hash sequence — and a serde roundtrip of level and
/// layout changes nothing.
#[test]
fn s20_determinism_hash_sequences() {
    let scenario = load("s14_chain_signal_crossing");

    let hashes = |level, layout| -> Vec<u64> {
        let mut sim = Sim::new(level, layout).expect("validates");
        let mut out = Vec::new();
        while sim.outcome().is_none() && sim.now() < MAX_TICKS {
            sim.step();
            out.push(sim.replay_hash());
        }
        out
    };

    let first = hashes(&scenario.level, &scenario.layout);
    let second = hashes(&scenario.level, &scenario.layout);
    assert_eq!(first, second, "two fresh runs must hash identically");

    let level_ron = ron::to_string(&scenario.level).expect("serialize level");
    let layout_ron = ron::to_string(&scenario.layout).expect("serialize layout");
    let level2: stellwerk_sim::Level = ron::from_str(&level_ron).expect("roundtrip level");
    let layout2: stellwerk_sim::Layout = ron::from_str(&layout_ron).expect("roundtrip layout");
    let third = hashes(&level2, &layout2);
    assert_eq!(first, third, "serde roundtrip must not change behavior");
}

/// Golden final hashes of every scenario (16 runnable ones; s12 is a
/// reachability check and s20 is the sequence test above).
// Re-blessed 2026-06-12: `passed_switches` was removed from the per-tick
// canonical bytes (it grew unbounded and made hashing O(crossings) per
// tick; the chained hash already commits to it via the path history).
// Every scenario outcome stayed identical — only the hash values moved.
const GOLD: &[(&str, u64)] = &[
    ("s01_single_train_straight", 0xaf009bb080fffc4b),
    ("s02_curves_and_diagonals", 0x8f4c8dd015854ae9),
    ("s03_two_trains_block_signal", 0x76e4f3a9471309ce),
    ("s04_rear_end_no_signal", 0x5cc1b9c3f396555c),
    ("s05_head_on_single_track", 0x9931a1bac59d3afc),
    ("s06_passing_loop", 0xd020f46f840e0064),
    // s07 == s08 is correct: the switch CONFIG differs, but both runs
    // produce the identical state history (config is static, not state).
    ("s07_switch_default", 0xa5ebc1d563ac379f),
    ("s08_switch_dest_rule", 0xa5ebc1d563ac379f),
    ("s09_switch_rule_order", 0x43e6d27bfb7748ce),
    ("s10_misrouting_wrong_sink", 0xd1b72535c8dd39f3),
    ("s11_misrouting_dead_end", 0xbe092f768c924f9d),
    ("s13_block_only_crossing", 0x2043e36fb1968d67),
    ("s14_chain_signal_crossing", 0x059d8791202a235a),
    ("s15_chain_reservation_timing", 0xc9d5f0fd701e4917),
    ("s16_source_fifo", 0x70fd35b8304896bf),
    ("s17_long_train_two_blocks", 0x09eda40416d89b8b),
    ("s18_ring_self_jam", 0xace23424b3abf55c),
    ("s19_full_scoring", 0xd4f8cb459f46bac5),
];

#[test]
fn golden_replay_hashes() {
    let mut failures = Vec::new();
    for &(name, expected) in GOLD {
        let scenario = load(name);
        let mut sim = Sim::new(&scenario.level, &scenario.layout).expect("validates");
        sim.run(MAX_TICKS);
        let got = sim.replay_hash();
        if got != expected {
            failures.push(format!("    (\"{name}\", 0x{got:016x}),"));
        }
    }
    assert!(
        failures.is_empty(),
        "golden hash mismatches — if the behavior change is intended, bless:\n{}",
        failures.join("\n")
    );
}
