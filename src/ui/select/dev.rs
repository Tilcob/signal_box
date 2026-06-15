//! Dev authoring (optimierung/07): per-level delete, full progress reset, and a
//! two-click "delete all". Only compiled with `feature = "dev"`.

use bevy::prelude::*;

use crate::font::UiFont;
use crate::i18n::set_lang;
use crate::levels::{Catalog, Progress, load_catalog};

use super::{
    DevDeleteAll, DevDeleteArmed, DevDeleteLevel, DevResetProgress, OpenChapter, UiSelect, UiStatus,
    rebuild_select,
};

/// After any mutation the catalog and i18n table are reloaded and the screen is
/// rebuilt in place (dropping back to the overview, since indices shift).
#[allow(clippy::too_many_arguments)]
pub(super) fn dev_select_actions(
    per_level: Query<(&Interaction, &DevDeleteLevel), Changed<Interaction>>,
    reset: Query<&Interaction, (Changed<Interaction>, With<DevResetProgress>)>,
    delete_all: Query<&Interaction, (Changed<Interaction>, With<DevDeleteAll>)>,
    roots: Query<Entity, With<UiSelect>>,
    ui_font: Res<UiFont>,
    mut catalog: ResMut<Catalog>,
    mut progress: ResMut<Progress>,
    mut status: ResMut<UiStatus>,
    mut armed: ResMut<DevDeleteArmed>,
    mut open: ResMut<OpenChapter>,
    mut commands: Commands,
) {
    let mut dirty = false;
    let mut catalog_changed = false;

    for (interaction, target) in &per_level {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let id = target.0.clone();
        crate::authoring::delete_level(&id);
        progress.levels.remove(&id);
        progress.save();
        status.0 = format!("Level gelöscht: {id}");
        armed.0 = false;
        dirty = true;
        catalog_changed = true;
    }

    if reset.iter().any(|i| *i == Interaction::Pressed) {
        let lang = progress.lang.clone();
        *progress = Progress::default();
        progress.lang = lang;
        progress.save();
        status.0 = "Fortschritt zurückgesetzt (Gleise/Weichen/Scores aller Level).".into();
        armed.0 = false;
        dirty = true;
    }

    if delete_all.iter().any(|i| *i == Interaction::Pressed) {
        if armed.0 {
            let ids: Vec<String> = catalog.0.iter().map(|e| e.id.clone()).collect();
            for id in &ids {
                crate::authoring::delete_level(id);
                progress.levels.remove(id);
            }
            progress.save();
            status.0 = format!("{} Level von der Platte gelöscht.", ids.len());
            armed.0 = false;
            dirty = true;
            catalog_changed = true;
        } else {
            armed.0 = true;
            status.0 = "! Nochmal \"ALLE Level löschen\" klicken zum Bestätigen.".into();
        }
    }

    if !dirty {
        return;
    }
    // Level deletes remove i18n keys; reload the live table for the current
    // language so the rebuilt screen reflects the change.
    let lang = if progress.lang.is_empty() {
        "de"
    } else {
        &progress.lang
    };
    set_lang(lang);
    if catalog_changed {
        *catalog = load_catalog();
    }
    // Indices shift after a catalog change — drop back to the overview.
    open.0 = None;
    rebuild_select(&mut commands, &roots, &ui_font.0, &catalog, &progress, None, &status.0);
}
