//! App state machine and the shared resources of the slice.
//!
//! Mode switching is the Zachlike contract (GDD §5): Edit and Run are
//! strictly separate states; Result shows the outcome and leads back.

use bevy::prelude::*;
use stellwerk_sim::Outcome;
use stellwerk_sim::grid::{Cell, Dir8};
use stellwerk_sim::layout::Layout;
use stellwerk_sim::level::Level;

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameState {
    /// Title screen — the app boots here. "Start" leads into [`Loading`].
    #[default]
    MainMenu,
    /// Asset/catalog load gate, shown as a loading screen. Auto-advances to
    /// [`LevelSelect`] once everything is resident (see `crate::loading`).
    Loading,
    LevelSelect,
    /// Size picker before creating a new sandbox (M2 §2.2).
    SandboxSetup,
    Edit,
    Run,
    Result,
}

/// The level being played, plus its catalog identity. In the sandbox the
/// level itself is editable (sources, sinks, schedule — M2 plan §2.2).
#[derive(Resource, Clone)]
pub struct ActiveLevel {
    pub id: String,
    pub index: usize,
    pub level: Level,
    /// Authored briefing (operating order, GDD §8.1); localized at render via
    /// `i18n::briefing`. Empty in the sandbox.
    pub briefing: String,
    pub sandbox: bool,
}

/// Outcome of the last run, displayed by the Result overlay.
#[derive(Resource, Clone)]
pub struct LastOutcome(pub Outcome);

/// The player's build, with full undo/redo (plan M1 §2: every build action
/// is an invertible operation — the same op vocabulary carries the M2
/// sharing format).
#[derive(Resource)]
pub struct Editor {
    pub layout: Layout,
    pub undo: Vec<crate::editor::EditOp>,
    pub redo: Vec<crate::editor::EditOp>,
    pub tool: Tool,
    /// R/T-rotated placement variant for the switch and signal tools (signed
    /// so both rotation directions work; indexed via `rem_euclid`).
    pub variant: i32,
    /// The two connectors of the track ghost. R/T rotate it (8 orientations);
    /// the radial RMB menu picks a new exit (the curve/secondary forms).
    pub track_form: (Dir8, Dir8),
    /// Cells visited by the current track drag.
    pub drag: Option<Vec<Cell>>,
    /// Switch cell whose config panel is open.
    pub selected_switch: Option<Cell>,
    /// Cell whose radial track menu is open (RMB on the Track tool).
    pub radial: Option<Cell>,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            layout: Layout::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            tool: Tool::default(),
            variant: 0,
            track_form: (Dir8::W, Dir8::E),
            drag: None,
            selected_switch: None,
            radial: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Select,
    Track,
    Switch,
    SignalBlock,
    SignalChain,
    Erase,
    /// Sandbox only: place a source / sink on a connector.
    Source,
    Sink,
}

impl Tool {
    /// Every variant, for exhaustive iteration (the i18n coverage checker
    /// walks these to assert each tool name resolves in both languages).
    #[cfg(test)]
    pub const ALL: [Tool; 8] = [
        Tool::Select,
        Tool::Track,
        Tool::Switch,
        Tool::SignalBlock,
        Tool::SignalChain,
        Tool::Erase,
        Tool::Source,
        Tool::Sink,
    ];
}

/// Live validation + reachability results for the current build.
#[derive(Resource, Default)]
pub struct Diagnostics {
    pub errors: Vec<stellwerk_sim::ValidationError>,
    pub unreachable: Vec<stellwerk_sim::Unreachable>,
}

impl Diagnostics {
    pub fn start_allowed(&self) -> bool {
        self.errors.is_empty()
    }
}

pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .init_resource::<Editor>()
            .init_resource::<Diagnostics>();
    }
}
