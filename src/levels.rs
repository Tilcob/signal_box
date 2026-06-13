//! Level catalog (RON files under `assets/levels/`), local progress with
//! solution slots (Save v2, M2 plan §3) and the sandbox level file.
//!
//! Save location: the platform config directory via `directories`
//! (GDD §12.2). A progress file from M1 in the working directory is
//! migrated once, read-only — the old file stays untouched.

use bevy::prelude::*;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use stellwerk_sim::Score;
use stellwerk_sim::grid::Cell;
use stellwerk_sim::layout::Layout;
use stellwerk_sim::level::{Level, Par};
use stellwerk_sim::units::Tick;

pub const SOLUTION_SLOTS: usize = 3;

pub struct LevelEntry {
    /// File stem, e.g. `k1_02_blocktakt` — the stable progress key.
    pub id: String,
    pub level: Level,
}

#[derive(Resource)]
pub struct Catalog(pub Vec<LevelEntry>);

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct LevelProgress {
    pub solved: bool,
    /// Best value per axis — across solutions, like GDD §7.7.
    pub best_throughput: Option<u64>,
    pub best_material: Option<u32>,
    pub best_lateness: Option<u64>,
    /// Autosaved build (slot 0 in spirit; kept separate for compatibility).
    pub layout: Layout,
    /// Manual solution slots (GDD §7.7: mehrere Lösungs-Slots).
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
        .map(|levels| Progress {
            levels,
            lang: String::new(),
            degraded: false,
        })
}

// --- Sandbox ----------------------------------------------------------------

pub const SANDBOX_ID: &str = "sandbox";

pub fn sandbox_template() -> Level {
    let mut buildable = Vec::new();
    for x in 0..12 {
        for y in -3..4 {
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
            .and_then(|text| ron::from_str::<Level>(&text).map_err(|e| e.to_string()))
        {
            Ok(level) => entries.push(LevelEntry { id, level }),
            Err(e) => error!("level {path:?} unreadable: {e}"),
        }
    }
    entries.sort_by(|a, b| a.id.cmp(&b.id));
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
