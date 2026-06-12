//! Live validation: full error report plus reachability warnings, refreshed
//! whenever the build or the level changes.

use bevy::prelude::*;
use stellwerk_sim::{check_reachability, validate};

use crate::state::{ActiveLevel, Diagnostics, Editor};

pub(super) fn revalidate(
    active: Option<Res<ActiveLevel>>,
    editor: Res<Editor>,
    mut diagnostics: ResMut<Diagnostics>,
) {
    let Some(active) = active else { return };
    if !editor.is_changed() && !active.is_changed() {
        return;
    }
    diagnostics.errors = validate(&active.level, &editor.layout);
    diagnostics.unreachable = if diagnostics.errors.is_empty() {
        check_reachability(&active.level, &editor.layout).unwrap_or_default()
    } else {
        Vec::new()
    };
}
