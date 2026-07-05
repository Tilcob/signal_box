//! # stellwerk_codes — sharing codes
//!
//! Solutions and levels travel as short text codes through Discord, Reddit
//! and forums — no server, no workshop. Format: `SW1-` prefix, then base64
//! over `[version byte ‖ postcard bytes]`. The version byte is checked
//! BEFORE deserialization, so codes survive format evolution: an old client
//! rejects a newer code with a clear error instead of garbage.
//!
//! Robustness is the contract here: `decode` must never panic on arbitrary
//! input — every malformed code is a future community bug report.

use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use serde::{Deserialize, Serialize};
use stellwerk_sim::Layout;
use stellwerk_sim::Level;

/// Human-recognizable prefix (shows up in forum posts).
pub const PREFIX: &str = "SW1-";
/// Format version; bump on every breaking payload change.
///
/// v1 → v2: [`stellwerk_sim::level::SourceDef`] gained a `label`.
/// v2 → v3: [`stellwerk_sim::layout::SignalDef`] gained a `priority`.
/// v3 → v4: freight — [`stellwerk_sim::level::Level`] gained `platforms` and
/// [`stellwerk_sim::level::ScheduleEntry`] gained `stop`. Because v1/v2/v3
/// decoders read the *live* `ScheduleEntry` positionally, the pre-freight shape
/// is now frozen as [`v3::ScheduleEntryV3`] and the old mirrors point at it.
/// Older codes are still decoded by the [`v1`]/[`v2`]/[`v3`] migrations (empty
/// platforms, `stop = None`), so the golden codes stay valid.
pub const VERSION: u8 = 4;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Payload {
    /// A build for a known (campaign) level.
    Solution { level_id: String, layout: Layout },
    /// A complete custom puzzle (sandbox export).
    Level { level: Level },
}

pub fn encode(payload: &Payload) -> String {
    let bytes = postcard::to_allocvec(payload).expect("payload types are serializable");
    let mut framed = Vec::with_capacity(bytes.len() + 1);
    framed.push(VERSION);
    framed.extend_from_slice(&bytes);
    format!("{PREFIX}{}", STANDARD_NO_PAD.encode(framed))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Not a Stellwerk code at all (missing prefix).
    Prefix,
    /// Prefix ok, but the base64 part is damaged.
    Base64,
    /// A code from a different (likely newer) game version.
    Version(u8),
    /// Version ok, payload bytes unreadable.
    Corrupt,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // English on purpose: this is the engine-/locale-agnostic Display for
        // logs and Debug. The player-facing, translated message is built in the
        // frontend (see `ui::select::decode_error_text`).
        match self {
            DecodeError::Prefix => write!(f, "not a Stellwerk code (prefix {PREFIX} missing)"),
            DecodeError::Base64 => write!(f, "code damaged (base64 unreadable)"),
            DecodeError::Version(v) => {
                write!(f, "code version {v} is unsupported (expected {VERSION})")
            }
            DecodeError::Corrupt => write!(f, "code damaged (contents unreadable)"),
        }
    }
}

pub fn decode(text: &str) -> Result<Payload, DecodeError> {
    // Forum copy-paste tolerance: surrounding whitespace and line breaks.
    let cleaned: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let body = cleaned.strip_prefix(PREFIX).ok_or(DecodeError::Prefix)?;
    let framed = STANDARD_NO_PAD
        .decode(body)
        .map_err(|_| DecodeError::Base64)?;
    let (&version, payload_bytes) = framed.split_first().ok_or(DecodeError::Corrupt)?;
    match version {
        VERSION => postcard::from_bytes(payload_bytes).map_err(|_| DecodeError::Corrupt),
        3 => postcard::from_bytes::<v3::PayloadV3>(payload_bytes)
            .map(v3::migrate)
            .map_err(|_| DecodeError::Corrupt),
        2 => postcard::from_bytes::<v2::PayloadV2>(payload_bytes)
            .map(v2::migrate)
            .map_err(|_| DecodeError::Corrupt),
        1 => postcard::from_bytes::<v1::PayloadV1>(payload_bytes)
            .map(v1::migrate)
            .map_err(|_| DecodeError::Corrupt),
        v => Err(DecodeError::Version(v)),
    }
}

/// Migration of v3 codes. v3 → v4 added freight (`Level.platforms`,
/// `ScheduleEntry.stop`). This mirror freezes the pre-freight `ScheduleEntry`
/// (8 fields, no `stop`) as [`ScheduleEntryV3`] and the pre-freight `Level`
/// shape as [`LevelV3`]; the v1/v2 mirrors reuse [`ScheduleEntryV3`] and
/// [`up_schedule`] since their schedule field is byte-identical. `Layout`,
/// `SourceDef`, `SinkDef` and `Par` are unchanged by freight and reused live.
mod v3 {
    use super::Payload;
    use serde::Deserialize;
    use stellwerk_sim::Layout;
    use stellwerk_sim::Level;
    use stellwerk_sim::grid::Cell;
    use stellwerk_sim::level::{Par, ScheduleEntry, SinkDef, SourceDef};
    use stellwerk_sim::units::{Len, SinkId, SourceId, Speed, Tick, TrainClass, TrainId};

    /// Frozen pre-freight [`ScheduleEntry`] — the 8 fields before `stop`. The
    /// live type gained `stop`, so v1/v2/v3 must decode against this copy.
    #[derive(Deserialize)]
    pub struct ScheduleEntryV3 {
        pub train: TrainId,
        pub class: TrainClass,
        pub length: Len,
        pub speed: Speed,
        pub source: SourceId,
        pub sink: SinkId,
        pub depart: Tick,
        pub due: Tick,
    }

    #[derive(Deserialize)]
    pub struct LevelV3 {
        pub name: String,
        pub buildable: Vec<Cell>,
        pub fixed: Layout,
        pub sources: Vec<SourceDef>,
        pub sinks: Vec<SinkDef>,
        pub schedule: Vec<ScheduleEntryV3>,
        pub par: Par,
    }

    /// Same variant order as [`Payload`] — postcard encodes the discriminant
    /// positionally, so the order is load-bearing for old codes.
    #[derive(Deserialize)]
    pub enum PayloadV3 {
        Solution { level_id: String, layout: Layout },
        Level { level: LevelV3 },
    }

    /// Lifts pre-freight schedule entries to the current shape (`stop = None`).
    /// Shared by the v1/v2/v3 migrations.
    pub fn up_schedule(schedule: Vec<ScheduleEntryV3>) -> Vec<ScheduleEntry> {
        schedule
            .into_iter()
            .map(|e| ScheduleEntry {
                train: e.train,
                class: e.class,
                length: e.length,
                speed: e.speed,
                source: e.source,
                sink: e.sink,
                depart: e.depart,
                due: e.due,
                stop: None,
            })
            .collect()
    }

    pub fn migrate(payload: PayloadV3) -> Payload {
        match payload {
            PayloadV3::Solution { level_id, layout } => Payload::Solution { level_id, layout },
            PayloadV3::Level { level } => Payload::Level {
                level: Level {
                    name: level.name,
                    buildable: level.buildable,
                    fixed: level.fixed,
                    sources: level.sources,
                    sinks: level.sinks,
                    platforms: vec![],
                    schedule: up_schedule(level.schedule),
                    par: level.par,
                },
            },
        }
    }
}

/// Migration of v2 codes. v2 → v3 added [`stellwerk_sim::layout::SignalDef`]'s
/// `priority`, so this mirror freezes the priority-less signal/layout shape and
/// up-migrates each signal to priority 0. `Layout` was identical in v1 and v2,
/// so [`v1`] reuses [`LayoutV2`]/[`up_layout`] from here. TrackPiece/SwitchDef
/// are unchanged since v1 and are reused as-is. The schedule uses the frozen
/// [`v3::ScheduleEntryV3`] (the live `ScheduleEntry` gained `stop` in v4).
mod v2 {
    use super::{Payload, v3};
    use serde::Deserialize;
    use stellwerk_sim::Level;
    use stellwerk_sim::grid::{Cell, Dir8};
    use stellwerk_sim::layout::{Layout, SignalDef, SignalKind, SwitchDef, TrackPiece};
    use stellwerk_sim::level::{Par, SinkDef, SourceDef};

    #[derive(Deserialize)]
    pub struct SignalDefV2 {
        pub cell: Cell,
        pub at: Dir8,
        pub kind: SignalKind,
    }

    #[derive(Deserialize)]
    pub struct LayoutV2 {
        pub pieces: Vec<TrackPiece>,
        pub switches: Vec<SwitchDef>,
        pub signals: Vec<SignalDefV2>,
    }

    #[derive(Deserialize)]
    pub struct LevelV2 {
        pub name: String,
        pub buildable: Vec<Cell>,
        pub fixed: LayoutV2,
        pub sources: Vec<SourceDef>,
        pub sinks: Vec<SinkDef>,
        pub schedule: Vec<v3::ScheduleEntryV3>,
        pub par: Par,
    }

    /// Same variant order as [`Payload`] — postcard encodes the discriminant
    /// positionally, so the order is load-bearing for old codes.
    #[derive(Deserialize)]
    pub enum PayloadV2 {
        Solution { level_id: String, layout: LayoutV2 },
        Level { level: LevelV2 },
    }

    /// Lifts a priority-less layout to the current one (signals default to
    /// priority 0). Shared by the v1 and v2 migrations.
    pub fn up_layout(layout: LayoutV2) -> Layout {
        Layout {
            pieces: layout.pieces,
            switches: layout.switches,
            signals: layout
                .signals
                .into_iter()
                .map(|s| SignalDef {
                    cell: s.cell,
                    at: s.at,
                    kind: s.kind,
                    priority: 0,
                })
                .collect(),
        }
    }

    pub fn migrate(payload: PayloadV2) -> Payload {
        match payload {
            PayloadV2::Solution { level_id, layout } => Payload::Solution {
                level_id,
                layout: up_layout(layout),
            },
            PayloadV2::Level { level } => Payload::Level {
                level: Level {
                    name: level.name,
                    buildable: level.buildable,
                    fixed: up_layout(level.fixed),
                    sources: level.sources,
                    sinks: level.sinks,
                    platforms: vec![],
                    schedule: v3::up_schedule(level.schedule),
                    par: level.par,
                },
            },
        }
    }
}

/// Migration of v1 codes. v1 differs from v2 only in a label-less
/// [`stellwerk_sim::level::SourceDef`]; the layout shape is v2's (priority-less),
/// so it reuses [`v2::LayoutV2`]/[`v2::up_layout`]. A v1 source up-migrates to an
/// empty label (rendered as `Q{id}`), each signal to priority 0.
mod v1 {
    use super::{Payload, v2, v3};
    use serde::Deserialize;
    use stellwerk_sim::Level;
    use stellwerk_sim::grid::{Cell, Dir8};
    use stellwerk_sim::level::{Par, SinkDef, SourceDef};
    use stellwerk_sim::units::SourceId;

    #[derive(Deserialize)]
    pub struct SourceDefV1 {
        pub id: SourceId,
        pub cell: Cell,
        pub dir: Dir8,
    }

    #[derive(Deserialize)]
    pub struct LevelV1 {
        pub name: String,
        pub buildable: Vec<Cell>,
        pub fixed: v2::LayoutV2,
        pub sources: Vec<SourceDefV1>,
        pub sinks: Vec<SinkDef>,
        pub schedule: Vec<v3::ScheduleEntryV3>,
        pub par: Par,
    }

    /// Same variant order as [`Payload`] — postcard encodes the discriminant
    /// positionally, so the order is load-bearing for old codes.
    #[derive(Deserialize)]
    pub enum PayloadV1 {
        Solution {
            level_id: String,
            layout: v2::LayoutV2,
        },
        Level {
            level: LevelV1,
        },
    }

    pub fn migrate(payload: PayloadV1) -> Payload {
        match payload {
            PayloadV1::Solution { level_id, layout } => Payload::Solution {
                level_id,
                layout: v2::up_layout(layout),
            },
            PayloadV1::Level { level } => Payload::Level {
                level: Level {
                    name: level.name,
                    buildable: level.buildable,
                    fixed: v2::up_layout(level.fixed),
                    sources: level
                        .sources
                        .into_iter()
                        .map(|s| SourceDef {
                            id: s.id,
                            cell: s.cell,
                            dir: s.dir,
                            label: String::new(),
                        })
                        .collect(),
                    sinks: level.sinks,
                    platforms: vec![],
                    schedule: v3::up_schedule(level.schedule),
                    par: level.par,
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellwerk_sim::grid::{Cell, Dir8};
    use stellwerk_sim::layout::{
        RuleWhen, SignalDef, SignalKind, SwitchDef, SwitchRule, TrackPiece,
    };
    use stellwerk_sim::level::{
        Level, Par, PlatformDef, PlatformStop, ScheduleEntry, SinkDef, SourceDef,
    };
    use stellwerk_sim::units::*;

    fn sample_layout() -> Layout {
        Layout {
            pieces: vec![TrackPiece {
                cell: Cell { x: 1, y: 0 },
                a: Dir8::W,
                b: Dir8::E,
            }],
            switches: vec![SwitchDef {
                cell: Cell { x: 2, y: 0 },
                stem: Dir8::W,
                branches: [Dir8::E, Dir8::NE],
                default_branch: 1,
                rules: vec![SwitchRule {
                    when: RuleWhen::DestIs(SinkId(1)),
                    branch: 0,
                }],
            }],
            signals: vec![SignalDef {
                cell: Cell { x: 1, y: 0 },
                at: Dir8::E,
                kind: SignalKind::Chain,
                priority: 0,
            }],
        }
    }

    fn sample_level() -> Level {
        Level {
            name: "Code-Test".into(),
            buildable: vec![Cell { x: 0, y: 0 }, Cell { x: 1, y: 0 }],
            fixed: Layout::default(),
            sources: vec![SourceDef {
                id: SourceId(0),
                cell: Cell { x: 0, y: 0 },
                dir: Dir8::W,
                label: "NORD".into(),
            }],
            sinks: vec![SinkDef {
                id: SinkId(0),
                cell: Cell { x: 1, y: 0 },
                dir: Dir8::E,
                label: "OST".into(),
            }],
            platforms: vec![],
            schedule: vec![ScheduleEntry {
                train: TrainId(0),
                class: TrainClass(0),
                length: Len(800),
                speed: Speed(100),
                source: SourceId(0),
                sink: SinkId(0),
                depart: Tick(0),
                due: Tick(50),
                stop: None,
            }],
            par: Par {
                throughput: Tick(40),
                material: 2,
                lateness: 0,
            },
        }
    }

    /// A freight level with a platform and a train that must stop there — the
    /// v4-only shape. Exercises the new positional fields end to end.
    fn sample_freight_level() -> Level {
        let mut level = sample_level();
        level.platforms = vec![PlatformDef {
            id: PlatformId(0),
            cell: Cell { x: 1, y: 0 },
            dir: Dir8::N,
            label: "RAMPE".into(),
        }];
        level.schedule[0].stop = Some(PlatformStop {
            platform: PlatformId(0),
            dwell: Tick(30),
        });
        level
    }

    #[test]
    fn solution_roundtrip() {
        let payload = Payload::Solution {
            level_id: "k1_02_blocktakt".into(),
            layout: sample_layout(),
        };
        assert_eq!(decode(&encode(&payload)), Ok(payload));
    }

    #[test]
    fn level_roundtrip() {
        let payload = Payload::Level {
            level: sample_level(),
        };
        assert_eq!(decode(&encode(&payload)), Ok(payload));
    }

    /// v4 freight fields (`platforms`, `ScheduleEntry.stop`) must survive a full
    /// roundtrip — not silently dropped to a default.
    #[test]
    fn freight_level_roundtrips() {
        let payload = Payload::Level {
            level: sample_freight_level(),
        };
        assert_eq!(decode(&encode(&payload)), Ok(payload));
    }

    /// Frozen v4 freight Level code — the first with `platforms` and a
    /// `ScheduleEntry.stop`. Must decode forever to the freight level; guards the
    /// new positional layout against silent reordering.
    #[test]
    fn v4_freight_golden_stays_decodable() {
        let golden =
            "SW1-BAEJQ29kZS1UZXN0AgAAAgAAAAABAAAABgROT1JEAQACAAIDT1NUAQACAAAFUkFNUEUBAADADMgBAAAAMgEAHigCAA";
        assert_eq!(
            decode(golden),
            Ok(Payload::Level {
                level: sample_freight_level(),
            })
        );
    }

    /// Frozen v3 Level code (version byte 3), encoded before freight existed
    /// from [`sample_level`]'s pre-freight shape. After the v4 field additions
    /// it must still decode — up-migrated to empty `platforms` and `stop = None`.
    /// If this breaks you changed how v3 is read — fix the [`v3`] mirror, don't
    /// re-bless.
    #[test]
    fn v3_level_code_migrates_to_no_freight() {
        let golden = "SW1-AwEJQ29kZS1UZXN0AgAAAgAAAAABAAAABgROT1JEAQACAAIDT1NUAQAAwAzIAQAAADIoAgA";
        let Ok(Payload::Level { level }) = decode(golden) else {
            panic!("v3 level code did not decode to a Level");
        };
        assert_eq!(level.name, "Code-Test");
        assert!(level.platforms.is_empty()); // up-migrated
        assert_eq!(level.schedule.len(), 1);
        assert_eq!(level.schedule[0].train, TrainId(0));
        assert_eq!(level.schedule[0].due, Tick(50));
        assert_eq!(level.schedule[0].stop, None); // up-migrated
    }

    /// A non-zero signal priority is part of the v3 wire format and must
    /// survive a full roundtrip (not silently lost to a default).
    #[test]
    fn signal_priority_roundtrips() {
        let mut layout = sample_layout();
        layout.signals[0].priority = 7;
        let payload = Payload::Solution {
            level_id: "prio".into(),
            layout,
        };
        assert_eq!(decode(&encode(&payload)), Ok(payload));
    }

    #[test]
    fn whitespace_tolerant() {
        let code = encode(&Payload::Solution {
            level_id: "x".into(),
            layout: Layout::default(),
        });
        let mangled = format!("  {}\n{}  \r\n", &code[..10], &code[10..]);
        assert!(decode(&mangled).is_ok());
    }

    #[test]
    fn garbage_never_panics() {
        for bad in [
            "",
            "hello",
            "SW1-",
            "SW1-!!!!",
            "SW1-AAAA",
            "SW2-AAAA",
            "SW1-AA==", // padding (we use no-pad)
            "SW1-\u{1F600}",
        ] {
            let _ = decode(bad); // any Err is fine, panic is not
        }
        assert_eq!(decode("hello"), Err(DecodeError::Prefix));
    }

    #[test]
    fn future_version_is_rejected_cleanly() {
        let mut framed = vec![99u8];
        framed.extend(
            postcard::to_allocvec(&Payload::Solution {
                level_id: "x".into(),
                layout: Layout::default(),
            })
            .unwrap(),
        );
        let code = format!("{PREFIX}{}", STANDARD_NO_PAD.encode(framed));
        assert_eq!(decode(&code), Err(DecodeError::Version(99)));
    }

    /// Frozen v1 Level code — sources had no `label` in v1. This is a real v1
    /// wire byte string (NOT re-encoded through the current mirror), so a future
    /// reordering of the `v1` mirror types is caught here instead of silently
    /// corrupting old codes. Built from: name "Alt", source (id 0, cell (0,0),
    /// W), sink (id 0, cell (1,0), E, "OST"), empty schedule, par(0,1,0). Must
    /// decode forever; if it breaks you changed how v1 is read — fix the mirror,
    /// don't re-bless.
    #[test]
    fn v1_level_code_migrates_to_empty_source_labels() {
        let golden = "SW1-AQEDQWx0AQAAAAAAAQAAAAYBAAIAAgNPU1QAAAEA";
        let Ok(Payload::Level { level }) = decode(golden) else {
            panic!("v1 level code did not decode to a Level");
        };
        assert_eq!(level.name, "Alt");
        assert_eq!(level.sources.len(), 1);
        assert_eq!(level.sources[0].id, SourceId(0));
        assert_eq!(level.sources[0].cell, Cell { x: 0, y: 0 });
        assert_eq!(level.sources[0].dir, Dir8::W);
        assert_eq!(level.sources[0].label, ""); // up-migrated from v1
        assert_eq!(level.sinks[0].label, "OST");
    }

    /// Frozen golden code: this exact string must decode forever — the
    /// regression guard against silent format breaks. If this
    /// fails you broke compatibility: bump VERSION and add a migration,
    /// don't re-bless.
    #[test]
    fn golden_code_stays_decodable() {
        let payload = Payload::Solution {
            level_id: "gold".into(),
            layout: sample_layout(),
        };
        let golden = "SW1-AQAEZ29sZAECAAYCAQQABgIBAQEAAQABAgACAQ";
        match decode(golden) {
            Ok(decoded) => assert_eq!(decoded, payload),
            Err(e) => panic!(
                "golden code undecodable ({e:?}) — current encoding would be:\n{}",
                encode(&payload)
            ),
        }
    }
}
