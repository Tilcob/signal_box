//! Level definition: what the designer provides and the player must serve.
//!
//! Split into two layers:
//!
//! - [`core`] — the frozen SIMULATION core ([`Level`] & friends). This, and
//!   only this, is what a sharing code carries (`stellwerk_codes`, postcard —
//!   a positional, byte-fragile format). Treat it as frozen: adding a field
//!   here shifts the byte layout, breaks every existing code and the
//!   golden-code test. Such a change needs a `stellwerk_codes::VERSION` bump
//!   plus a migration.
//! - [`meta`] — campaign organisation ([`LevelMeta`] / [`LevelDef`]): chapter,
//!   order, optional-hard, briefing. Lives only on disk (`assets/levels/*.ron`)
//!   and in the catalog, never in a code — so it may grow freely (additive
//!   fields with `#[serde(default)]`, versioned via [`LEVEL_SCHEMA_VERSION`]).

mod core;
mod meta;

pub use core::{Level, Par, PlatformDef, PlatformStop, ScheduleEntry, SinkDef, SourceDef};
pub use meta::{LEVEL_SCHEMA_VERSION, LevelDef, LevelMeta};
