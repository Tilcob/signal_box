//! App state machine and the shared resources of the slice.
//!
//! Mode switching is the Zachlike contract: Edit and Run are
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
    /// Size picker before creating a new sandbox.
    SandboxSetup,
    Edit,
    Run,
    Result,
}

/// The level being played, plus its catalog identity. In the sandbox the
/// level itself is editable (sources, sinks, schedule).
#[derive(Resource, Clone)]
pub struct ActiveLevel {
    pub id: String,
    pub index: usize,
    pub level: Level,
    /// Authored briefing (operating order); localized at render via
    /// `i18n::briefing`. Empty in the sandbox.
    pub briefing: String,
    pub sandbox: bool,
}

/// Outcome of the last run, displayed by the Result overlay.
#[derive(Resource, Clone)]
pub struct LastOutcome(pub Outcome);

/// Whether the in-level pause menu is open (Edit and Run). While true the
/// gameplay input is gated off and the pause overlay covers the screen; Esc
/// toggles it. See `crate::ui::pause`.
#[derive(Resource, Default)]
pub struct Paused(pub bool);

/// Run condition for the gameplay systems that must freeze while the pause
/// menu is open (edit tools, the sim tick, the start hotkey).
pub fn not_paused(paused: Res<Paused>) -> bool {
    !paused.0
}

/// The player's build, with full undo/redo. Every build action
/// is an invertible operation — the same op vocabulary carries the
/// sharing format.
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
    /// Signal (cell, connector) whose config panel is open. Mutually exclusive
    /// with `selected_switch` — the Select tool sets one and clears the other.
    pub selected_signal: Option<(Cell, Dir8)>,
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
            selected_signal: None,
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
    /// Sandbox only: toggle a cell between buildable and a non-buildable block.
    Block,
}

impl Tool {
    /// Every variant, for exhaustive iteration (the i18n coverage checker
    /// walks these to assert each tool name resolves in both languages).
    #[cfg(test)]
    pub const ALL: [Tool; 9] = [
        Tool::Select,
        Tool::Track,
        Tool::Switch,
        Tool::SignalBlock,
        Tool::SignalChain,
        Tool::Erase,
        Tool::Source,
        Tool::Sink,
        Tool::Block,
    ];
}

/// The numeric input field (schedule editor) that currently holds keyboard
/// focus, if any. While set, the edit hotkeys/board pointer/start key are
/// suppressed so typing digits doesn't leak into the game.
/// See `crate::ui::numeric_field`.
#[derive(Resource, Default)]
pub struct FocusedField(pub Option<Entity>);

/// Run condition: no numeric field is focused. Gates the systems that read raw
/// keyboard/mouse so they don't fight a field being typed into.
pub fn no_field_focused(focus: Res<FocusedField>) -> bool {
    focus.0.is_none()
}

/// A sandbox build problem the sim's `ValidationError` cannot express: a
/// runnable level needs at least one source AND one sink. Checked only in the
/// sandbox (campaign levels are authored with both) and blocks START like a
/// validation error. Computed in `crate::editor::validation`, localized in
/// `crate::ui::valerr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildIssue {
    NothingPlaced,
    MissingSource,
    MissingSink,
}

/// Live validation + reachability results for the current build.
#[derive(Resource, Default)]
pub struct Diagnostics {
    pub errors: Vec<stellwerk_sim::ValidationError>,
    pub unreachable: Vec<stellwerk_sim::Unreachable>,
    /// Sandbox-only build blocks (missing source/sink); empty for campaign levels.
    pub build_issues: Vec<BuildIssue>,
}

impl Diagnostics {
    pub fn start_allowed(&self) -> bool {
        self.errors.is_empty() && self.build_issues.is_empty()
    }
}

pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .init_resource::<Editor>()
            .init_resource::<Paused>()
            .init_resource::<FocusedField>()
            .init_resource::<Diagnostics>();
    }
}
