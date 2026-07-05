//! Edit mode: tools, drag track drawing, invertible edit ops with undo/redo,
//! live validation + reachability warnings (incl. sandbox
//! source/sink tools).
//!
//! Validation is never modal: faulty elements glow on the board, the start
//! switch stays locked while errors exist. Reachability problems are
//! warnings only — watching the misrouting happen is a lesson (Säule 4).
//!
//! Sandbox edits (source/sink placement, the schedule editor) mutate the
//! LEVEL rather than the layout, but share the SAME undo stack as layout ops
//! via [`ops::EditOp`]'s level variants — one Ctrl+Z timeline.

mod ops;
mod overlays;
mod placement;
mod tools;
mod validation;

pub use ops::{EditOp, do_op};
pub use validation::{BuildIssue, Diagnostics};

use bevy::prelude::*;
use stellwerk_sim::Layout;

use crate::state::{ActiveLevel, Editor, GameState, no_field_focused, not_paused, save_modal_closed};

/// `fixed ⊕ player` layout, rebuilt only when the build or level changes.
/// Both the pointer (placement gates) and the overlays (ghost colouring) read
/// it every frame — without the cache each rebuilt it independently, two full
/// `Layout` clones per frame for the whole time the editor was open.
#[derive(Resource, Default)]
pub(crate) struct MergedLayout(pub(crate) Layout);

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MergedLayout>()
            .init_resource::<Diagnostics>()
            .add_systems(
            Update,
            (
                sync_merged_layout,
                // Suppressed while a numeric field is focused, so typing digits
                // can't switch tools / undo, and a focus-blur click can't also
                // place track. Also frozen while the save-level modal is open.
                tools::hotkeys
                    .run_if(not_paused)
                    .run_if(no_field_focused)
                    .run_if(save_modal_closed),
                tools::cycle_track_form
                    .run_if(not_paused)
                    .run_if(no_field_focused)
                    .run_if(save_modal_closed),
                tools::rmb_cycle_curve
                    .run_if(not_paused)
                    .run_if(no_field_focused)
                    .run_if(save_modal_closed),
                tools::pointer
                    .run_if(not_paused)
                    .run_if(no_field_focused)
                    .run_if(save_modal_closed),
                overlays::draw_overlays.run_if(not_paused),
                validation::revalidate,
                // Esc opens/closes the pause menu in place of leaving the level.
                // Frozen while the save modal is open — there Esc closes the modal.
                crate::ui::pause::toggle_pause.run_if(save_modal_closed),
            )
                .chain()
                .run_if(in_state(GameState::Edit)),
        );
    }
}

/// Refreshes [`MergedLayout`] when the player build or the level changed.
/// Runs first in the edit chain; an edit made later this frame shows up in the
/// cache next frame (the ghost overlay catches up one frame later — invisible).
fn sync_merged_layout(
    active: Option<Res<ActiveLevel>>,
    editor: Res<Editor>,
    mut merged: ResMut<MergedLayout>,
) {
    let Some(active) = active else { return };
    if !editor.is_changed() && !active.is_changed() {
        return;
    }
    merged.0 = active.level.fixed.merged(&editor.layout);
}
