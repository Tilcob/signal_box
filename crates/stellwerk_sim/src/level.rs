//! Level definition: what the designer provides and the player must serve.
//! See GDD §7.5 (schedule), §7.7 (par values) and plan §3.4.

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
/// it feeds the punctuality score axis (GDD §7.5).
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

/// Designer reference values per score axis (GDD §7.7).
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
