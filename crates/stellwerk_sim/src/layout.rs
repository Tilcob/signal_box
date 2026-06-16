//! Player-built infrastructure (tracks, switches, signals) and its
//! validation.
//!
//! Design decisions are encoded here:
//! - **Open track ends are legal.** A dead end is a *runtime* misrouting
//!   failure, not a build error — and source/sink connectors
//!   are open by design.
//! - **Junctions require a switch.** Every connector point may be used by at
//!   most two stubs; three or more meetings without a switch are rejected.
//!   This keeps continuation at plain points unique forever.
//! - Validation returns the **complete list** of errors, not the first one.

use crate::grid::{Cell, Dir8, Point, pair_len};
use crate::level::Level;
use crate::units::TrainClass;
use crate::units::segment_lengths::MAX_SPEED_EXCLUSIVE;
use crate::units::{SinkId, SourceId, TrainId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// A plain track piece: connects two connectors of `cell` via the center.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackPiece {
    pub cell: Cell,
    pub a: Dir8,
    pub b: Dir8,
}

/// Routing rule of a switch: first matching rule wins, evaluated in list
/// order; without a match the default branch is taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwitchRule {
    pub when: RuleWhen,
    /// Index into [`SwitchDef::branches`] (0 or 1).
    pub branch: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleWhen {
    DestIs(SinkId),
    ClassIs(TrainClass),
}

/// A switch occupies its cell exclusively: one stem connector, two branch
/// connectors, all joined via the cell center.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwitchDef {
    pub cell: Cell,
    pub stem: Dir8,
    pub branches: [Dir8; 2],
    /// Index into `branches` used when no rule matches.
    pub default_branch: u8,
    pub rules: Vec<SwitchRule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalKind {
    Block,
    Chain,
}

/// A signal anchors at the `at` connector of `cell` and gates trains leaving
/// the cell across that connector (one travel direction only). The guarded
/// stop position is exactly the connector point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalDef {
    pub cell: Cell,
    pub at: Dir8,
    pub kind: SignalKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Layout {
    pub pieces: Vec<TrackPiece>,
    pub switches: Vec<SwitchDef>,
    pub signals: Vec<SignalDef>,
}

impl Layout {
    /// Combined designer + player infrastructure — the simulation input.
    pub fn merged(&self, other: &Layout) -> Layout {
        let mut out = self.clone();
        out.pieces.extend_from_slice(&other.pieces);
        out.switches.extend_from_slice(&other.switches);
        out.signals.extend_from_slice(&other.signals);
        out
    }

    /// All (cell, connector) uses, one entry per stub. Sorted, deterministic.
    pub(crate) fn all_stubs(&self) -> Vec<(Cell, Dir8)> {
        let mut stubs = Vec::new();
        for piece in &self.pieces {
            stubs.push((piece.cell, piece.a));
            stubs.push((piece.cell, piece.b));
        }
        for switch in &self.switches {
            stubs.push((switch.cell, switch.stem));
            stubs.push((switch.cell, switch.branches[0]));
            stubs.push((switch.cell, switch.branches[1]));
        }
        stubs.sort();
        stubs
    }

    /// Does any piece or switch in this layout use the given connector?
    /// Public: frontends use it to gate signal placement on existing track.
    pub fn has_stub(&self, cell: Cell, dir: Dir8) -> bool {
        self.pieces
            .iter()
            .any(|p| p.cell == cell && (p.a == dir || p.b == dir))
            || self
                .switches
                .iter()
                .any(|s| s.cell == cell && (s.stem == dir || s.branches.contains(&dir)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Connector pair is a kink (< 90° turn) or degenerate (a == b).
    IllegalPiecePair {
        cell: Cell,
        a: Dir8,
        b: Dir8,
    },
    DuplicatePiece {
        cell: Cell,
        a: Dir8,
        b: Dir8,
    },
    /// Stem and branches of a switch must be three distinct connectors.
    SwitchConnectorClash {
        cell: Cell,
    },
    /// A stem→branch turn sharper than 90°.
    SwitchBranchAngle {
        cell: Cell,
        branch: Dir8,
    },
    SwitchDefaultOutOfRange {
        cell: Cell,
    },
    SwitchRuleBranchOutOfRange {
        cell: Cell,
    },
    /// Switch rule references a sink id the level does not define.
    SwitchRuleUnknownSink {
        cell: Cell,
        sink: SinkId,
    },
    /// A switch cell must not contain plain pieces (and only one switch).
    SwitchCellNotExclusive {
        cell: Cell,
    },
    DuplicateSwitch {
        cell: Cell,
    },
    /// Signal anchored on a connector no track uses.
    SignalOffTrack {
        cell: Cell,
        at: Dir8,
    },
    DuplicateSignal {
        cell: Cell,
        at: Dir8,
    },
    /// Three or more stubs meet at a connector point without a switch.
    JunctionWithoutSwitch {
        point: Point,
    },
    /// Two pieces of the same cell use the same connector — a hairpin at the
    /// connector point; also required so each (cell, connector) maps to
    /// exactly one stub (signal anchoring relies on that).
    ConnectorReused {
        cell: Cell,
        dir: Dir8,
    },
    /// Player infrastructure outside the buildable area.
    OutsideBuildable {
        cell: Cell,
    },
    DuplicateSourceId {
        id: SourceId,
    },
    DuplicateSinkId {
        id: SinkId,
    },
    /// Source/sink connector has no track anchoring it.
    SourceOffTrack {
        id: SourceId,
    },
    SinkOffTrack {
        id: SinkId,
    },
    DuplicateTrainId {
        train: TrainId,
    },
    UnknownSource {
        train: TrainId,
        source: SourceId,
    },
    UnknownSink {
        train: TrainId,
        sink: SinkId,
    },
    NonPositiveLength {
        train: TrainId,
    },
    NonPositiveSpeed {
        train: TrainId,
    },
    /// Anti-tunneling: speed must stay below the shortest possible edge
    /// (one cardinal stub), or a train could skip a stop point in one tick.
    SpeedTooHigh {
        train: TrainId,
    },
    DueBeforeDepart {
        train: TrainId,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ValidationError::*;
        match self {
            IllegalPiecePair { cell, a, b } => write!(
                f,
                "illegal track pair {a:?}-{b:?} in cell ({}, {}) — turns sharper than 90° are kinks",
                cell.x, cell.y
            ),
            DuplicatePiece { cell, a, b } => write!(
                f,
                "duplicate track piece {a:?}-{b:?} in cell ({}, {})",
                cell.x, cell.y
            ),
            SwitchConnectorClash { cell } => write!(
                f,
                "switch in cell ({}, {}) needs three distinct connectors",
                cell.x, cell.y
            ),
            SwitchBranchAngle { cell, branch } => write!(
                f,
                "switch branch {branch:?} in cell ({}, {}) turns sharper than 90° from the stem",
                cell.x, cell.y
            ),
            SwitchDefaultOutOfRange { cell } => write!(
                f,
                "switch default branch in cell ({}, {}) must be 0 or 1",
                cell.x, cell.y
            ),
            SwitchRuleBranchOutOfRange { cell } => write!(
                f,
                "switch rule branch in cell ({}, {}) must be 0 or 1",
                cell.x, cell.y
            ),
            SwitchRuleUnknownSink { cell, sink } => write!(
                f,
                "switch rule in cell ({}, {}) references unknown sink {}",
                cell.x, cell.y, sink.0
            ),
            SwitchCellNotExclusive { cell } => write!(
                f,
                "cell ({}, {}) holds a switch and other track — switch cells are exclusive",
                cell.x, cell.y
            ),
            DuplicateSwitch { cell } => {
                write!(f, "two switches in cell ({}, {})", cell.x, cell.y)
            }
            SignalOffTrack { cell, at } => write!(
                f,
                "signal at connector {at:?} of cell ({}, {}) sits on no track",
                cell.x, cell.y
            ),
            DuplicateSignal { cell, at } => write!(
                f,
                "duplicate signal at connector {at:?} of cell ({}, {})",
                cell.x, cell.y
            ),
            JunctionWithoutSwitch { point } => write!(
                f,
                "three or more tracks meet at lattice point ({}, {}) — junctions need a switch",
                point.x, point.y
            ),
            ConnectorReused { cell, dir } => write!(
                f,
                "connector {dir:?} of cell ({}, {}) is used by more than one piece",
                cell.x, cell.y
            ),
            OutsideBuildable { cell } => write!(
                f,
                "player track in cell ({}, {}) is outside the buildable area",
                cell.x, cell.y
            ),
            DuplicateSourceId { id } => write!(f, "duplicate source id {}", id.0),
            DuplicateSinkId { id } => write!(f, "duplicate sink id {}", id.0),
            SourceOffTrack { id } => {
                write!(f, "source {} has no track anchoring its connector", id.0)
            }
            SinkOffTrack { id } => write!(f, "sink {} has no track anchoring its connector", id.0),
            DuplicateTrainId { train } => {
                write!(f, "duplicate train id {}", train.0)
            }
            UnknownSource { train, source } => write!(
                f,
                "train {} departs from unknown source {}",
                train.0, source.0
            ),
            UnknownSink { train, sink } => {
                write!(f, "train {} targets unknown sink {}", train.0, sink.0)
            }
            NonPositiveLength { train } => {
                write!(f, "train {} has non-positive length", train.0)
            }
            NonPositiveSpeed { train } => {
                write!(f, "train {} has non-positive speed", train.0)
            }
            SpeedTooHigh { train } => write!(
                f,
                "train {} is faster than the shortest edge ({MAX_SPEED_EXCLUSIVE} LE/tick) — tunneling risk",
                train.0
            ),
            DueBeforeDepart { train } => {
                write!(f, "train {} is due before it departs", train.0)
            }
        }
    }
}

/// Validates the level plus the player layout as a whole. Returns *all*
/// errors found; an empty vector means the pair is simulation-ready.
pub fn validate(level: &Level, player: &Layout) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let merged = level.fixed.merged(player);

    // --- Pieces -----------------------------------------------------------
    let mut seen_pieces: BTreeSet<(Cell, Dir8, Dir8)> = BTreeSet::new();
    for piece in &merged.pieces {
        let (lo, hi) = if piece.a.index() <= piece.b.index() {
            (piece.a, piece.b)
        } else {
            (piece.b, piece.a)
        };
        if pair_len(piece.a, piece.b).is_none() {
            errors.push(ValidationError::IllegalPiecePair {
                cell: piece.cell,
                a: piece.a,
                b: piece.b,
            });
        }
        if !seen_pieces.insert((piece.cell, lo, hi)) {
            errors.push(ValidationError::DuplicatePiece {
                cell: piece.cell,
                a: lo,
                b: hi,
            });
        }
    }

    // --- Switches ----------------------------------------------------------
    let sink_ids: BTreeSet<SinkId> = level.sinks.iter().map(|s| s.id).collect();
    let mut switch_cells: BTreeSet<Cell> = BTreeSet::new();
    for switch in &merged.switches {
        let [b0, b1] = switch.branches;
        if switch.stem == b0 || switch.stem == b1 || b0 == b1 {
            errors.push(ValidationError::SwitchConnectorClash { cell: switch.cell });
        }
        for branch in [b0, b1] {
            if branch != switch.stem && pair_len(switch.stem, branch).is_none() {
                errors.push(ValidationError::SwitchBranchAngle {
                    cell: switch.cell,
                    branch,
                });
            }
        }
        if switch.default_branch > 1 {
            errors.push(ValidationError::SwitchDefaultOutOfRange { cell: switch.cell });
        }
        for rule in &switch.rules {
            if rule.branch > 1 {
                errors.push(ValidationError::SwitchRuleBranchOutOfRange { cell: switch.cell });
            }
            if let RuleWhen::DestIs(sink) = rule.when
                && !sink_ids.contains(&sink)
            {
                errors.push(ValidationError::SwitchRuleUnknownSink {
                    cell: switch.cell,
                    sink,
                });
            }
        }
        if !switch_cells.insert(switch.cell) {
            errors.push(ValidationError::DuplicateSwitch { cell: switch.cell });
        }
        if merged.pieces.iter().any(|p| p.cell == switch.cell) {
            errors.push(ValidationError::SwitchCellNotExclusive { cell: switch.cell });
        }
    }

    // --- Connector reuse within a cell + junction rule ---------------------
    let all_stubs = merged.all_stubs(); // sorted
    let mut reuse_reported: BTreeSet<(Cell, Dir8)> = BTreeSet::new();
    for pair in all_stubs.windows(2) {
        if pair[0] == pair[1] && reuse_reported.insert(pair[0]) {
            errors.push(ValidationError::ConnectorReused {
                cell: pair[0].0,
                dir: pair[0].1,
            });
        }
    }
    let mut stubs_per_point: BTreeMap<Point, u32> = BTreeMap::new();
    for (cell, dir) in all_stubs {
        *stubs_per_point
            .entry(cell.connector_point(dir))
            .or_insert(0) += 1;
    }
    for (point, count) in &stubs_per_point {
        if *count > 2 {
            errors.push(ValidationError::JunctionWithoutSwitch { point: *point });
        }
    }

    // --- Signals ------------------------------------------------------------
    let mut seen_signals: BTreeSet<(Cell, Dir8)> = BTreeSet::new();
    for signal in &merged.signals {
        if !merged.has_stub(signal.cell, signal.at) {
            errors.push(ValidationError::SignalOffTrack {
                cell: signal.cell,
                at: signal.at,
            });
        }
        if !seen_signals.insert((signal.cell, signal.at)) {
            errors.push(ValidationError::DuplicateSignal {
                cell: signal.cell,
                at: signal.at,
            });
        }
    }

    // --- Buildable area (player infrastructure only) ------------------------
    let buildable: BTreeSet<Cell> = level.buildable.iter().copied().collect();
    let player_cells = player
        .pieces
        .iter()
        .map(|p| p.cell)
        .chain(player.switches.iter().map(|s| s.cell))
        .chain(player.signals.iter().map(|s| s.cell));
    let mut reported: BTreeSet<Cell> = BTreeSet::new();
    for cell in player_cells {
        if !buildable.contains(&cell) && reported.insert(cell) {
            errors.push(ValidationError::OutsideBuildable { cell });
        }
    }

    // --- Sources / sinks ------------------------------------------------------
    let mut source_ids: BTreeSet<SourceId> = BTreeSet::new();
    for source in &level.sources {
        if !source_ids.insert(source.id) {
            errors.push(ValidationError::DuplicateSourceId { id: source.id });
        }
        if !merged.has_stub(source.cell, source.dir) {
            errors.push(ValidationError::SourceOffTrack { id: source.id });
        }
    }
    let mut seen_sink_ids: BTreeSet<SinkId> = BTreeSet::new();
    for sink in &level.sinks {
        if !seen_sink_ids.insert(sink.id) {
            errors.push(ValidationError::DuplicateSinkId { id: sink.id });
        }
        if !merged.has_stub(sink.cell, sink.dir) {
            errors.push(ValidationError::SinkOffTrack { id: sink.id });
        }
    }

    // --- Schedule ---------------------------------------------------------------
    let mut train_ids: BTreeSet<TrainId> = BTreeSet::new();
    for entry in &level.schedule {
        if !train_ids.insert(entry.train) {
            errors.push(ValidationError::DuplicateTrainId { train: entry.train });
        }
        if !source_ids.contains(&entry.source) {
            errors.push(ValidationError::UnknownSource {
                train: entry.train,
                source: entry.source,
            });
        }
        if !sink_ids.contains(&entry.sink) {
            errors.push(ValidationError::UnknownSink {
                train: entry.train,
                sink: entry.sink,
            });
        }
        if entry.length.0 <= 0 {
            errors.push(ValidationError::NonPositiveLength { train: entry.train });
        }
        if entry.speed.0 <= 0 {
            errors.push(ValidationError::NonPositiveSpeed { train: entry.train });
        } else if entry.speed.0 >= MAX_SPEED_EXCLUSIVE {
            errors.push(ValidationError::SpeedTooHigh { train: entry.train });
        }
        if entry.due < entry.depart {
            errors.push(ValidationError::DueBeforeDepart { train: entry.train });
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{Par, ScheduleEntry, SinkDef, SourceDef};
    use crate::units::{Len, Speed, Tick};

    fn cell(x: i32, y: i32) -> Cell {
        Cell { x, y }
    }

    /// Straight W–E line over `n` cells with one source, one sink, one train.
    fn straight_level(n: i32) -> (Level, Layout) {
        let layout = Layout {
            pieces: (0..n)
                .map(|x| TrackPiece {
                    cell: cell(x, 0),
                    a: Dir8::W,
                    b: Dir8::E,
                })
                .collect(),
            switches: vec![],
            signals: vec![],
        };
        let level = Level {
            name: "test".into(),
            buildable: (0..n).map(|x| cell(x, 0)).collect(),
            fixed: Layout::default(),
            sources: vec![SourceDef {
                id: SourceId(0),
                cell: cell(0, 0),
                dir: Dir8::W,
            }],
            sinks: vec![SinkDef {
                id: SinkId(0),
                cell: cell(n - 1, 0),
                dir: Dir8::E,
                label: "OST".into(),
            }],
            schedule: vec![ScheduleEntry {
                train: TrainId(0),
                class: TrainClass(0),
                length: Len(800),
                speed: Speed(100),
                source: SourceId(0),
                sink: SinkId(0),
                depart: Tick(0),
                due: Tick(200),
            }],
            par: Par {
                throughput: Tick(200),
                material: 6,
                lateness: 0,
            },
        };
        (level, layout)
    }

    #[test]
    fn valid_level_has_no_errors() {
        let (level, layout) = straight_level(4);
        assert_eq!(validate(&level, &layout), vec![]);
    }

    #[test]
    fn kink_is_rejected() {
        let (level, mut layout) = straight_level(4);
        layout.pieces.push(TrackPiece {
            cell: cell(0, 1),
            a: Dir8::N,
            b: Dir8::NE,
        });
        // The kink cell is also outside the buildable strip — both reported.
        let errors = validate(&level, &layout);
        assert!(errors.contains(&ValidationError::IllegalPiecePair {
            cell: cell(0, 1),
            a: Dir8::N,
            b: Dir8::NE,
        }));
        assert!(errors.contains(&ValidationError::OutsideBuildable { cell: cell(0, 1) }));
    }

    #[test]
    fn junction_without_switch_is_rejected() {
        // Corner points border four cells — three diagonals from three
        // different cells meet at the corner (2, 4) of the point lattice.
        let (mut level, mut layout) = straight_level(4);
        level.buildable.extend([cell(0, 1), cell(1, 2), cell(1, 1)]);
        layout.pieces.extend([
            TrackPiece {
                cell: cell(0, 1),
                a: Dir8::SW,
                b: Dir8::NE,
            },
            TrackPiece {
                cell: cell(1, 2),
                a: Dir8::SW,
                b: Dir8::NE,
            },
            TrackPiece {
                cell: cell(1, 1),
                a: Dir8::NW,
                b: Dir8::SE,
            },
        ]);
        let expected_point = cell(0, 1).connector_point(Dir8::NE);
        assert_eq!(
            validate(&level, &layout),
            vec![ValidationError::JunctionWithoutSwitch {
                point: expected_point
            }]
        );
    }

    #[test]
    fn connector_reuse_within_a_cell_is_rejected() {
        let (level, mut layout) = straight_level(4);
        // (0,0) already has W–E; a second piece reusing W is a hairpin.
        layout.pieces.push(TrackPiece {
            cell: cell(0, 0),
            a: Dir8::W,
            b: Dir8::NE,
        });
        let errors = validate(&level, &layout);
        assert!(
            errors.contains(&ValidationError::ConnectorReused {
                cell: cell(0, 0),
                dir: Dir8::W,
            }),
            "got: {errors:?}"
        );
    }

    #[test]
    fn dead_ends_are_legal() {
        let (mut level, mut layout) = straight_level(4);
        // A stub track going nowhere: legal — misrouting is a runtime
        // failure, not a build error.
        level.buildable.push(cell(0, 1));
        layout.pieces.push(TrackPiece {
            cell: cell(0, 1),
            a: Dir8::W,
            b: Dir8::E,
        });
        assert_eq!(validate(&level, &layout), vec![]);
    }

    #[test]
    fn signal_off_track_is_rejected() {
        let (level, mut layout) = straight_level(4);
        layout.signals.push(SignalDef {
            cell: cell(1, 0),
            at: Dir8::N,
            kind: SignalKind::Block,
        });
        assert_eq!(
            validate(&level, &layout),
            vec![ValidationError::SignalOffTrack {
                cell: cell(1, 0),
                at: Dir8::N,
            }]
        );
    }

    #[test]
    fn schedule_errors_are_reported() {
        let (mut level, layout) = straight_level(4);
        let entry = &mut level.schedule[0];
        entry.speed = Speed(500); // == shortest edge → tunneling risk
        entry.due = Tick(0);
        entry.depart = Tick(10);
        let errors = validate(&level, &layout);
        assert!(errors.contains(&ValidationError::SpeedTooHigh { train: TrainId(0) }));
        assert!(errors.contains(&ValidationError::DueBeforeDepart { train: TrainId(0) }));
    }

    #[test]
    fn switch_validation() {
        let (mut level, mut layout) = straight_level(2);
        level.buildable.push(cell(2, 0));
        // Sharp branch: stem W, branch NW is a kink.
        layout.switches.push(SwitchDef {
            cell: cell(2, 0),
            stem: Dir8::W,
            branches: [Dir8::E, Dir8::NW],
            default_branch: 0,
            rules: vec![SwitchRule {
                when: RuleWhen::DestIs(SinkId(99)),
                branch: 1,
            }],
        });
        let errors = validate(&level, &layout);
        assert!(errors.contains(&ValidationError::SwitchBranchAngle {
            cell: cell(2, 0),
            branch: Dir8::NW,
        }));
        assert!(errors.contains(&ValidationError::SwitchRuleUnknownSink {
            cell: cell(2, 0),
            sink: SinkId(99),
        }));
    }
}
