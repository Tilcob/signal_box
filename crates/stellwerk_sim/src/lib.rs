//! # stellwerk_sim — deterministic simulation core for Stellwerk
//!
//! No engine, no rendering — pure data and rules. The Bevy frontend talks to
//! this crate exclusively through its public API (see plan §4.1; GDD §12.1).
//!
//! ## Determinism contract (plan §4.5 — binding for every commit)
//!
//! 1. No `f32`/`f64` and no `HashMap`/`HashSet` in simulation state or in
//!    any iteration that mutates state — use `BTreeMap`/`Vec` with a fixed
//!    sort order instead.
//! 2. All loops over trains/signals/blocks iterate in ascending id order.
//! 3. The replay hash is hand-rolled FNV-1a-64 over explicit canonical
//!    serialization of the state (`std`'s hashers are process-seeded and
//!    unsuitable).
//! 4. Every length/speed/timing constant lives in [`units`] and is frozen
//!    after M0 — changing one invalidates every replay hash and the best score.
//! 5. No randomness. Frontend juice may use `rand`; this crate never does.
//!
//! Integer overflow: `overflow-checks = true` is enabled for all profiles in
//! the workspace root, so arithmetic bugs fail loudly and identically on
//! every platform instead of wrapping silently.

pub mod blocks;
pub mod graph;
pub mod grid;
pub mod layout;
pub mod level;
pub mod units;

pub use graph::{TrackGraph, build};
pub use layout::{Layout, ValidationError, validate};
pub use level::Level;
