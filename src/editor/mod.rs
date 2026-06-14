//! Edit mode: tools, drag track drawing, invertible edit ops with undo/redo,
//! live validation + reachability warnings (plan M1 §2/§3; M2: sandbox
//! source/sink tools).
//!
//! Validation is never modal: faulty elements glow on the board, the start
//! switch stays locked while errors exist. Reachability problems are
//! warnings only — watching the misrouting happen is a lesson (Säule 4).
//!
//! Sandbox note (M2-minimal): source/sink placement and the schedule editor
//! mutate the LEVEL, not the layout — they sit outside the undo stack.

mod ops;
mod overlays;
mod placement;
mod radial;
mod tools;
mod validation;

pub use ops::{EditOp, do_op};

use bevy::prelude::*;
use stellwerk_sim::Layout;

use crate::state::{ActiveLevel, Editor, GameState};

/// `fixed ⊕ player` layout, rebuilt only when the build or level changes.
/// Both the pointer (placement gates) and the overlays (ghost colouring) read
/// it every frame — without the cache each rebuilt it independently, two full
/// `Layout` clones per frame for the whole time the editor was open.
#[derive(Resource, Default)]
pub(crate) struct MergedLayout(pub(crate) Layout);

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MergedLayout>().add_systems(
            Update,
            (
                sync_merged_layout,
                tools::hotkeys,
                tools::pointer,
                overlays::draw_overlays,
                validation::revalidate,
                tools::leave_to_select,
                // After leave_to_select: it yields to an open menu, so the
                // menu must still be open when it runs — close happens here.
                radial::radial_menu,
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
