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
use std::path::PathBuf;
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
        let path = progress_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match ron::ser::to_string_pretty(self, Default::default()) {
            Ok(text) => {
                if let Err(e) = std::fs::write(&path, text) {
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
            // does not exist. Any other error (permissions, lock) must
            // not silently fall back: the next save() would overwrite
            // the real progress with defaults.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                match std::fs::read_to_string("stellwerk_progress.ron") {
                    Ok(text) => text,
                    Err(_) => return Progress::default(),
                }
            }
            Err(e) => {
                warn!("progress file {path:?} unreadable ({e}), starting with defaults");
                return Progress::default();
            }
        };
        // Robustness contract: corrupt file → defaults plus warning, never a
        // panic, never overwriting on mere read.
        match ron::from_str::<Progress>(&text) {
            Ok(progress) => progress,
            Err(_) => match ron::from_str::<BTreeMap<String, LevelProgress>>(&text) {
                // M1 format: bare map without the wrapper struct.
                Ok(levels) => Progress {
                    levels,
                    lang: String::new(),
                },
                Err(e) => {
                    warn!("progress file unreadable, starting fresh: {e}");
                    Progress::default()
                }
            },
        }
    }
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
