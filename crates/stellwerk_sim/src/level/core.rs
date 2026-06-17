//! The frozen simulation core of a level. **Byte-stable on purpose:** these
//! types are serialized into sharing codes via postcard (`stellwerk_codes`),
//! which is positional and has no field names — any change reorders the bytes
//! and invalidates every code in the wild. Add nothing here without a
//! `stellwerk_codes::VERSION` bump and a migration. Campaign metadata that is
//! NOT part of the playable puzzle belongs in [`super::meta`] instead.

use crate::grid::{Cell, Dir8};
use crate::layout::Layout;
use crate::units::{Len, SinkId, SourceId, Speed, Tick, TrainClass, TrainId};
use serde::{Deserialize, Serialize};

/// Trains enter the world here: they appear crossing into `cell` through the
/// `dir` connector. The cell must have track using that connector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceDef {
    pub id: SourceId,
    pub cell: Cell,
    pub dir: Dir8,
    /// Display name (station label on the board / schedule). Empty falls back
    /// to `Q{id}` at render. `serde(default)` keeps level files written before
    /// this field existed parseable; sharing codes carry it positionally and
    /// are gated by `stellwerk_codes::VERSION` (a v1 code has no source label
    /// and is up-migrated to an empty one).
    #[serde(default)]
    pub label: String,
}

/// Trains leave the world here: arrival is the head reaching the `dir`
/// connector of `cell`. The cell must have track using that connector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SinkDef {
    pub id: SinkId,
    pub cell: Cell,
    pub dir: Dir8,
    /// Display name (station label on the panel).
    pub label: String,
}

/// One timetable entry. `due` is the target arrival tick — lateness beyond
/// it feeds the punctuality score axis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleEntry {
    pub train: TrainId,
    pub class: TrainClass,
    pub length: Len,
    pub speed: Speed,
    pub source: SourceId,
    pub sink: SinkId,
    pub depart: Tick,
    pub due: Tick,
}

/// Designer reference values per score axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Par {
    pub throughput: Tick,
    pub material: u32,
    pub lateness: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Level {
    pub name: String,
    /// Cells the player may build on. Fixed track is exempt.
    pub buildable: Vec<Cell>,
    /// Designer-placed infrastructure (not removable by the player).
    pub fixed: Layout,
    pub sources: Vec<SourceDef>,
    pub sinks: Vec<SinkDef>,
    pub schedule: Vec<ScheduleEntry>,
    pub par: Par,
}
