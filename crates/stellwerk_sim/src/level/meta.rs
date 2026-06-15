//! Campaign metadata: how a level is organised INTO the campaign —
//! chapter, order, the optional-hard marker and the authored briefing.
//!
//! None of this is part of the playable puzzle, and crucially none of it ever
//! enters a sharing code: codes carry only the frozen [`Level`] core (see
//! [`super::core`]). [`LevelDef`] is the on-disk authoring shape — the catalog
//! reads it and hands the simulation the bare [`Level`]. Because metadata is
//! code-free, fields here may be added freely (additive + `#[serde(default)]`),
//! and breaking changes are gated by [`LEVEL_SCHEMA_VERSION`] — a concern
//! entirely separate from `stellwerk_codes::VERSION`.

use super::Level;
use serde::{Deserialize, Serialize};

/// On-disk schema version of a level file. Bump **only** on a breaking change
/// to the authoring format (a renamed/removed field, or a changed meaning).
/// Purely additive fields stay compatible through `#[serde(default)]` and need
/// no bump. This is *not* `stellwerk_codes::VERSION`: that one versions the
/// postcard wire format and is fed only by the frozen [`Level`] core, which
/// this layer deliberately leaves untouched.
pub const LEVEL_SCHEMA_VERSION: u16 = 1;

fn default_schema_version() -> u16 {
    LEVEL_SCHEMA_VERSION
}

/// Campaign organisation for one level. Never serialized into a sharing code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelMeta {
    /// On-disk format version (see [`LEVEL_SCHEMA_VERSION`]). Defaulted so a
    /// file written before the field existed still loads.
    #[serde(default = "default_schema_version")]
    pub schema_version: u16,
    /// Chapter this level belongs to (1-based). Drives grouping in
    /// the level select and, later, chapter unlock ("N solved opens the next").
    pub chapter: u8,
    /// Sort order WITHIN the chapter. Deliberately decoupled from the file
    /// stem: the stem is the stable progress + sharing-code key (`level_id`),
    /// so renaming it to renumber would break saves and codes. Authoring in
    /// steps of 10 leaves room to insert a level between two others later.
    pub order: u16,
    /// One of a chapter's last "optional-hard" levels: shown and
    /// playable, but never a progression blocker.
    #[serde(default)]
    pub optional_hard: bool,
    /// Authored briefing in the style of an operating order (1–2
    /// sentences). This German text is the i18n fallback for the
    /// `level.<id>.briefing` key — see the frontend's `i18n::briefing`.
    #[serde(default)]
    pub briefing: String,
}

impl Default for LevelMeta {
    fn default() -> Self {
        LevelMeta {
            schema_version: LEVEL_SCHEMA_VERSION,
            chapter: 0,
            order: 0,
            optional_hard: false,
            briefing: String::new(),
        }
    }
}

/// On-disk authoring shape of a campaign level: the playable [`Level`] core
/// plus its campaign [`LevelMeta`]. Files under `assets/levels/*.ron`
/// deserialize into this; the catalog then splits it into the parts the rest
/// of the game uses. The sandbox and sharing codes use a bare [`Level`] — they
/// have no campaign metadata by design.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelDef {
    pub meta: LevelMeta,
    pub sim: Level,
}
