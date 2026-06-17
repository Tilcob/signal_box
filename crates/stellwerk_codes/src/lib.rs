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
/// v1 → v2: [`stellwerk_sim::level::SourceDef`] gained a `label`. Older codes
/// are still decoded by the [`v1`] migration (sources up-migrate to an empty
/// label, rendered as `Q{id}`), so the golden v1 code stays valid.
pub const VERSION: u8 = 2;

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
        1 => postcard::from_bytes::<v1::PayloadV1>(payload_bytes)
            .map(v1::migrate)
            .map_err(|_| DecodeError::Corrupt),
        v => Err(DecodeError::Version(v)),
    }
}

/// Migration of `VERSION - 1` codes. Only [`stellwerk_sim::level::SourceDef`]
/// changed between v1 and v2 (it gained `label`), so the mirror reuses every
/// other current type and differs solely in a label-less source. A v1 source
/// up-migrates to an empty label (rendered as `Q{id}`).
mod v1 {
    use super::{Layout, Payload};
    use serde::Deserialize;
    use stellwerk_sim::Level;
    use stellwerk_sim::grid::{Cell, Dir8};
    use stellwerk_sim::level::{Par, ScheduleEntry, SinkDef, SourceDef};
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
        pub fixed: Layout,
        pub sources: Vec<SourceDefV1>,
        pub sinks: Vec<SinkDef>,
        pub schedule: Vec<ScheduleEntry>,
        pub par: Par,
    }

    /// Same variant order as [`Payload`] — postcard encodes the discriminant
    /// positionally, so the order is load-bearing for old codes.
    #[derive(Deserialize)]
    pub enum PayloadV1 {
        Solution { level_id: String, layout: Layout },
        Level { level: LevelV1 },
    }

    pub fn migrate(payload: PayloadV1) -> Payload {
        match payload {
            PayloadV1::Solution { level_id, layout } => Payload::Solution { level_id, layout },
            PayloadV1::Level { level } => Payload::Level {
                level: Level {
                    name: level.name,
                    buildable: level.buildable,
                    fixed: level.fixed,
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
                    schedule: level.schedule,
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
    use stellwerk_sim::level::{Level, Par, ScheduleEntry, SinkDef, SourceDef};
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
            schedule: vec![ScheduleEntry {
                train: TrainId(0),
                class: TrainClass(0),
                length: Len(800),
                speed: Speed(100),
                source: SourceId(0),
                sink: SinkId(0),
                depart: Tick(0),
                due: Tick(50),
            }],
            par: Par {
                throughput: Tick(40),
                material: 2,
                lateness: 0,
            },
        }
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
