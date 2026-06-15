//! Level catalog (RON files under `assets/levels/`), local progress with
//! solution slots (Save v2) and the sandbox level file.
//!
//! Save location: the platform config directory via `directories`.
//! A progress file from M1 in the working directory is
//! migrated once, read-only — the old file stays untouched.

use bevy::prelude::*;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use stellwerk_sim::Score;
use stellwerk_sim::grid::Cell;
use stellwerk_sim::layout::Layout;
use stellwerk_sim::level::{Level, LevelDef, LevelMeta, Par};
use stellwerk_sim::units::Tick;

pub const SOLUTION_SLOTS: usize = 3;

pub struct LevelEntry {
    /// File stem, e.g. `k1_02_blocktakt` — the stable progress key.
    pub id: String,
    /// Campaign metadata (chapter/order/optional-hard/briefing). Code-free.
    pub meta: LevelMeta,
    pub level: Level,
}

#[derive(Resource)]
pub struct Catalog(pub Vec<LevelEntry>);

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct LevelProgress {
    pub solved: bool,
    /// Best value per axis — across solutions.
    pub best_throughput: Option<u64>,
    pub best_material: Option<u32>,
    pub best_lateness: Option<u64>,
    /// Autosaved build (slot 0 in spirit; kept separate for compatibility).
    pub layout: Layout,
    /// Manual solution slots (multiple solutions per level).
    #[serde(default)]
    pub slots: Vec<Option<Layout>>,
}

impl LevelProgress {
    pub fn record(&mut self, score: &Score) {
        self.solved = true;
        let better = |best: &mut Option<u64>, v: u64| {
            *best = Some(best.map_or(v, |b| b.min(v)));
        };
        better(&mut self.best_throughput, score.throughput.0);
        better(&mut self.best_lateness, score.lateness);
        self.best_material = Some(
            self.best_material
                .map_or(score.material, |b| b.min(score.material)),
        );
    }

    pub fn medals(&self, level: &Level) -> [bool; 3] {
        [
            self.best_throughput
                .is_some_and(|v| v <= level.par.throughput.0),
            self.best_material.is_some_and(|v| v <= level.par.material),
            self.best_lateness.is_some_and(|v| v <= level.par.lateness),
        ]
    }

    pub fn slot(&self, index: usize) -> Option<&Layout> {
        self.slots.get(index).and_then(|s| s.as_ref())
    }

    pub fn set_slot(&mut self, index: usize, layout: Layout) {
        // Defensive: callers only pass `0..SOLUTION_SLOTS`, but this struct is
        // also deserialized from on-disk saves a player could hand-edit — an
        // out-of-range index must never index-panic.
        if index >= SOLUTION_SLOTS {
            return;
        }
        if self.slots.len() < SOLUTION_SLOTS {
            self.slots.resize(SOLUTION_SLOTS, None);
        }
        self.slots[index] = Some(layout);
    }
}

#[derive(Resource, Serialize, Deserialize, Default)]
pub struct Progress {
    pub levels: BTreeMap<String, LevelProgress>,
    /// UI language ("de"/"en"), persisted with the save.
    #[serde(default)]
    pub lang: String,
    /// Set when [`Progress::load`] had to fall back to defaults because an
    /// EXISTING file could not be read/parsed. The next [`Progress::save`]
    /// then preserves that original file as a `.bak` before overwriting, so a
    /// transient lock or a corrupt file never silently destroys real data.
    /// Not persisted.
    #[serde(skip)]
    degraded: bool,
}

fn config_dir() -> Option<PathBuf> {
    ProjectDirs::from("", "", "Stellwerk").map(|dirs| dirs.config_dir().to_path_buf())
}

fn progress_path() -> PathBuf {
    config_dir()
        .map(|dir| dir.join("progress.ron"))
        .unwrap_or_else(|| PathBuf::from("stellwerk_progress.ron"))
}

fn sandbox_path() -> PathBuf {
    config_dir()
        .map(|dir| dir.join("sandbox.ron"))
        .unwrap_or_else(|| PathBuf::from("stellwerk_sandbox.ron"))
}

impl Progress {
    pub fn entry(&mut self, id: &str) -> &mut LevelProgress {
        self.levels.entry(id.to_string()).or_default()
    }

    pub fn save(&self) {
        self.save_to(&progress_path());
    }

    /// Writes the progress to `path`. If this instance is [`Progress::degraded`]
    /// (a previous load could not read/parse an existing file), the existing
    /// file is first preserved as `<path>.bak` — but only once, so the very
    /// first (original) version is the one kept.
    fn save_to(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if self.degraded {
            let backup = path.with_extension("ron.bak");
            if path.exists() && !backup.exists() {
                if let Err(e) = std::fs::copy(path, &backup) {
                    warn!("cannot back up unreadable progress to {backup:?}: {e}");
                } else {
                    warn!("preserved unreadable progress as {backup:?} before overwriting");
                }
            }
        }
        match ron::ser::to_string_pretty(self, Default::default()) {
            Ok(text) => {
                if let Err(e) = std::fs::write(path, text) {
                    warn!("cannot write progress {path:?}: {e}");
                }
            }
            Err(e) => warn!("cannot serialize progress: {e}"),
        }
    }

    fn load() -> Progress {
        let path = progress_path();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            // One-time migration: read (not move) the M1 file from the
            // working directory — but only when the new file genuinely
            // does not exist.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                match std::fs::read_to_string("stellwerk_progress.ron") {
                    Ok(text) => text,
                    Err(_) => return Progress::default(),
                }
            }
            // The file exists but cannot be read (lock, permissions). Start
            // with defaults but mark degraded: the next save preserves the
            // original instead of clobbering it.
            Err(e) => {
                warn!("progress file {path:?} unreadable ({e}), starting with defaults");
                return Progress {
                    degraded: true,
                    ..Progress::default()
                };
            }
        };
        // Corrupt file → defaults, but degraded so the original is preserved
        // on the next save. Never panic, never overwrite on a mere read.
        match parse_progress(&text) {
            Some(progress) => progress,
            None => {
                warn!("progress file unreadable, starting fresh (original kept as .bak)");
                Progress {
                    degraded: true,
                    ..Progress::default()
                }
            }
        }
    }
}

/// Parses progress text, accepting the current wrapped format and the bare M1
/// map (one-time migration). `None` = neither parses. Pure (no filesystem) so
/// the format/migration contract is unit-testable.
fn parse_progress(text: &str) -> Option<Progress> {
    if let Ok(progress) = ron::from_str::<Progress>(text) {
        return Some(progress);
    }
    // M1 format: bare map without the wrapper struct.
    ron::from_str::<BTreeMap<String, LevelProgress>>(text)
        .ok()
        .map(|mut levels| {
            // Save v1 had no solution slots. Promote each M1 autosave build
            // into Slot 1 so it survives the volatile `layout` autosave — the
            // next run/Esc overwrites that field with the current editor build,
            // which would otherwise lose the player's only copy of an M1
            // solution. Never-built levels (empty layout) are skipped.
            for entry in levels.values_mut() {
                let empty = entry.layout.pieces.is_empty()
                    && entry.layout.switches.is_empty()
                    && entry.layout.signals.is_empty();
                if !empty {
                    entry.set_slot(0, entry.layout.clone());
                }
            }
            Progress {
                levels,
                lang: String::new(),
                degraded: false,
            }
        })
}

// --- Sandbox ----------------------------------------------------------------

pub const SANDBOX_ID: &str = "sandbox";

/// Default sandbox: the historical 12×7 area.
pub const SANDBOX_DEFAULT_W: u32 = 12;
pub const SANDBOX_DEFAULT_H: u32 = 7;
/// Lower bound: anything smaller cannot hold a real run (source + track + sink).
pub const SANDBOX_MIN: u32 = 3;
/// Upper bound, tied to the level-code budget: the largest empty
/// area must still encode to a Level-Code under the compression threshold
/// (~1500 chars). At ~2 bytes/cell (postcard) plus base64's 4/3
/// expansion, a square `SANDBOX_MAX`×`SANDBOX_MAX` area (~484 cells) lands near
/// 1300 chars. Guarded by `largest_empty_sandbox_fits_code_budget`.
pub const SANDBOX_MAX: u32 = 22;

/// An empty sandbox area of size `w`×`h`, **centered on (0,0)**:
/// the start camera sits at (0,0), so every size lands in view. `w`/`h` are
/// clamped to `[SANDBOX_MIN, SANDBOX_MAX]` — callers need not validate.
pub fn empty_sandbox(w: u32, h: u32) -> Level {
    let w = w.clamp(SANDBOX_MIN, SANDBOX_MAX) as i32;
    let h = h.clamp(SANDBOX_MIN, SANDBOX_MAX) as i32;
    // Centering: x0 = -(w/2). For w=12 → x0=-6, x in -6..6 (12 cells). For h=7
    // → y0=-3, y in -3..4 — identical to the historical y axis.
    let x0 = -(w / 2);
    let y0 = -(h / 2);
    let mut buildable = Vec::with_capacity((w * h) as usize);
    for x in x0..x0 + w {
        for y in y0..y0 + h {
            buildable.push(Cell { x, y });
        }
    }
    Level {
        name: "Sandbox".into(),
        buildable,
        fixed: Layout::default(),
        sources: Vec::new(),
        sinks: Vec::new(),
        schedule: Vec::new(),
        par: Par {
            throughput: Tick(0),
            material: 0,
            lateness: 0,
        },
    }
}

/// Default template (fallback in [`load_sandbox`]).
pub fn sandbox_template() -> Level {
    empty_sandbox(SANDBOX_DEFAULT_W, SANDBOX_DEFAULT_H)
}

pub fn load_sandbox() -> Level {
    std::fs::read_to_string(sandbox_path())
        .ok()
        .and_then(|text| ron::from_str(&text).ok())
        .unwrap_or_else(sandbox_template)
}

pub fn save_sandbox(level: &Level) {
    let path = sandbox_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match ron::ser::to_string_pretty(level, Default::default()) {
        Ok(text) => {
            if let Err(e) = std::fs::write(&path, text) {
                warn!("cannot write sandbox {path:?}: {e}");
            }
        }
        Err(e) => warn!("cannot serialize sandbox: {e}"),
    }
}

/// Reads every `assets/levels/*.ron` into the catalog. Called from the
/// `Loading` state (not at plugin build) so the on-demand load gate has real
/// work — see [`crate::loading`]. Public for that module.
pub fn load_catalog() -> Catalog {
    let dir = PathBuf::from("assets/levels");
    let mut entries: Vec<LevelEntry> = Vec::new();
    let read_dir = match std::fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(e) => {
            error!("cannot read {dir:?}: {e}");
            return Catalog(entries);
        }
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("ron") {
            continue;
        }
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| ron::from_str::<LevelDef>(&text).map_err(|e| e.to_string()))
        {
            Ok(def) => entries.push(LevelEntry {
                id,
                meta: def.meta,
                level: def.sim,
            }),
            Err(e) => error!("level {path:?} unreadable: {e}"),
        }
    }
    // Play/display order is (chapter, order) — decoupled from the file stem so
    // levels can be inserted without renaming (the stem stays the stable
    // progress/code key). The stem breaks ties for deterministic ordering.
    entries.sort_by(|a, b| {
        (a.meta.chapter, a.meta.order, &a.id).cmp(&(b.meta.chapter, b.meta.order, &b.id))
    });
    info!("{} levels loaded", entries.len());
    Catalog(entries)
}

pub struct LevelsPlugin;

impl Plugin for LevelsPlugin {
    fn build(&self, app: &mut App) {
        // Progress + language are needed EAGERLY: the main menu and loading
        // screen render translated text from frame one. The level catalog,
        // by contrast, is only needed from `LevelSelect` on and is therefore
        // loaded later in the `Loading` state (see `crate::loading`).
        let progress = Progress::load();
        let lang = if progress.lang.is_empty() {
            "de"
        } else {
            &progress.lang
        };
        crate::i18n::set_lang(lang);
        app.insert_resource(progress);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unique scratch directory for the filesystem tests; removed by the
    /// caller. Avoids `tempfile` as a dependency.
    fn scratch() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("stellwerk_test_{}_{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn empty_sandbox_has_w_times_h_cells_centered() {
        let lvl = empty_sandbox(12, 7);
        assert_eq!(lvl.buildable.len(), 12 * 7);
        let xs: Vec<i32> = lvl.buildable.iter().map(|c| c.x).collect();
        let ys: Vec<i32> = lvl.buildable.iter().map(|c| c.y).collect();
        // Centered: x in -6..6, y in -3..4 (matches the historical y axis).
        assert_eq!(xs.iter().min(), Some(&-6));
        assert_eq!(xs.iter().max(), Some(&5));
        assert_eq!(ys.iter().min(), Some(&-3));
        assert_eq!(ys.iter().max(), Some(&3));
    }

    #[test]
    fn empty_sandbox_clamps_out_of_range() {
        let lvl = empty_sandbox(0, 1000);
        assert_eq!(lvl.buildable.len() as u32, SANDBOX_MIN * SANDBOX_MAX);
    }

    /// Guards `SANDBOX_MAX` against the level-code budget: the
    /// largest empty area must still encode under the ~1500-char compression
    /// threshold. If this breaks, shrink `SANDBOX_MAX` / the presets.
    #[test]
    fn largest_empty_sandbox_fits_code_budget() {
        let level = empty_sandbox(SANDBOX_MAX, SANDBOX_MAX);
        let code = stellwerk_codes::encode(&stellwerk_codes::Payload::Level { level });
        assert!(
            code.len() < 1500,
            "largest empty sandbox code is {} chars",
            code.len()
        );
    }

    #[test]
    fn parse_current_format_roundtrips() {
        let mut p = Progress {
            lang: "en".into(),
            ..Progress::default()
        };
        p.entry("k1_01").solved = true;
        let text = ron::ser::to_string_pretty(&p, Default::default()).unwrap();
        let back = parse_progress(&text).expect("parses");
        assert_eq!(back.lang, "en");
        assert!(back.levels.get("k1_01").is_some_and(|l| l.solved));
    }

    /// Save-v2 migration: an M1 file is a BARE map without the wrapper struct.
    /// Frozen string — must keep migrating forever (it is what real M1 saves
    /// on disk look like).
    #[test]
    fn parse_migrates_m1_bare_map() {
        let m1 = r#"{
            "k1_01_erste_fahrt": (
                solved: true,
                best_throughput: Some(120),
                best_material: Some(8),
                best_lateness: Some(0),
                layout: (pieces: [], switches: [], signals: []),
            ),
        }"#;
        let p = parse_progress(m1).expect("M1 bare map migrates");
        assert_eq!(p.lang, "", "M1 had no language field");
        let entry = p.levels.get("k1_01_erste_fahrt").expect("level migrated");
        assert!(entry.solved);
        assert_eq!(entry.best_throughput, Some(120));
        // Empty M1 layout is NOT promoted — nothing to preserve.
        assert!(entry.slot(0).is_none(), "empty build leaves Slot 1 empty");
    }

    /// Save-v2 promotion: a real M1 autosave (the winning build lives in
    /// `layout`) is lifted into Slot 1 on migration, so the volatile autosave
    /// — overwritten on the next run/Esc — cannot become the only copy.
    /// Frozen string: what a real M1 save of a built level looks like on disk.
    #[test]
    fn m1_autosave_is_promoted_to_slot_one() {
        let m1 = r#"{
            "k1_01_erste_fahrt": (
                solved: true,
                best_throughput: Some(120),
                best_material: Some(4),
                best_lateness: Some(0),
                layout: (
                    pieces: [(cell: (x: 2, y: 0), a: W, b: E)],
                    switches: [],
                    signals: [],
                ),
            ),
        }"#;
        let p = parse_progress(m1).expect("M1 migrates");
        let entry = p.levels.get("k1_01_erste_fahrt").expect("level migrated");
        let slot = entry.slot(0).expect("M1 build promoted to Slot 1");
        assert_eq!(slot.pieces.len(), 1, "the M1 build landed in Slot 1");
        assert_eq!(slot.pieces[0].cell, Cell { x: 2, y: 0 });
        // Autosave left intact — nothing lost.
        assert_eq!(entry.layout.pieces.len(), 1);
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(parse_progress("this is not ron").is_none());
        assert!(parse_progress("").is_none());
    }

    #[test]
    fn degraded_save_backs_up_original_once() {
        let dir = scratch();
        let path = dir.join("progress.ron");
        std::fs::write(&path, "ORIGINAL CORRUPT CONTENT").unwrap();

        // A degraded load (corrupt file) → next save preserves the original.
        let degraded = Progress {
            degraded: true,
            lang: "de".into(),
            ..Progress::default()
        };
        degraded.save_to(&path);

        let bak = path.with_extension("ron.bak");
        assert!(bak.exists(), "original preserved as .bak");
        assert_eq!(
            std::fs::read_to_string(&bak).unwrap(),
            "ORIGINAL CORRUPT CONTENT"
        );
        // The new file was written and is valid progress.
        assert!(parse_progress(&std::fs::read_to_string(&path).unwrap()).is_some());

        // A SECOND degraded save must not overwrite the preserved original.
        std::fs::write(&path, "second version").unwrap();
        degraded.save_to(&path);
        assert_eq!(
            std::fs::read_to_string(&bak).unwrap(),
            "ORIGINAL CORRUPT CONTENT",
            "first backup is kept"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clean_save_makes_no_backup() {
        let dir = scratch();
        let path = dir.join("progress.ron");
        std::fs::write(&path, "EXISTING").unwrap();

        // A normal (non-degraded) save just overwrites, no .bak.
        Progress::default().save_to(&path);
        assert!(!path.with_extension("ron.bak").exists());

        std::fs::remove_dir_all(&dir).ok();
    }
}
