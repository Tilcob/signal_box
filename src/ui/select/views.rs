//! The two level-select views — the chapter overview and a single chapter's
//! level list — plus chapter navigation and launching a level.

use bevy::prelude::*;

#[cfg(feature = "dev")]
use crate::ui::widgets::small_button;
use crate::ui::widgets::{
    BUTTON_BG, BUTTON_BG_PRIMARY, MEDAL, SOLVED, TEXT_BRIGHT, TEXT_DIM, button, button_row, dot,
    text_bundle,
};
use crate::font::UiFont;
use crate::i18n::{level_name, t, t_or};
use crate::levels::{Catalog, LevelEntry, Progress};
use crate::state::{Editor, GameState};
use crate::ui::enter_level;
use crate::ui::encyclopedia::HelpButton;

#[cfg(feature = "dev")]
use super::{DevDeleteAll, DevDeleteLevel, DevOpenSandbox, DevResetProgress};
use super::{
    BackButton, ChapterButton, ImportButton, LangButton, LevelButton, NewSandboxButton, OpenChapter,
    SandboxButton, UiSelect, UiStatus, rebuild_select,
};

/// Localized chapter name (authored, fallback "Kapitel N" — like level names,
/// so an unauthored chapter never breaks).
fn chapter_name(chapter: u8) -> String {
    t_or(&format!("chapter.{chapter}.name"), &format!("Kapitel {chapter}"))
}

/// Home view: one button per campaign chapter (with a solved/total summary),
/// plus the global sandbox/import/language/help row and dev controls.
pub(super) fn build_overview(
    root: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    catalog: &Catalog,
    progress: &Progress,
) {
    root.spawn(text_bundle(font, t("select.title"), 30.0, TEXT_BRIGHT));
    root.spawn(text_bundle(font, t("select.chapter_hint"), 14.0, TEXT_DIM));

    // Distinct chapters in catalog order (the catalog is sorted by chapter).
    let mut chapters: Vec<u8> = Vec::new();
    for entry in &catalog.0 {
        if !chapters.contains(&entry.meta.chapter) {
            chapters.push(entry.meta.chapter);
        }
    }
    for ch in chapters {
        let total = catalog.0.iter().filter(|e| e.meta.chapter == ch).count();
        let solved = catalog
            .0
            .iter()
            .filter(|e| e.meta.chapter == ch)
            .filter(|e| progress.levels.get(&e.id).is_some_and(|p| p.solved))
            .count();
        let label = format!("{}   {solved}/{total}", chapter_name(ch));
        button(root, font, &label, BUTTON_BG, ChapterButton(ch));
    }

    root.spawn(Node {
        flex_direction: FlexDirection::Row,
        margin: UiRect::top(Val::Px(10.0)),
        ..default()
    })
    .with_children(|row| {
        button(row, font, &t("select.sandbox"), BUTTON_BG_PRIMARY, SandboxButton);
        button(row, font, &t("select.new_sandbox"), BUTTON_BG, NewSandboxButton);
        button(row, font, &t("select.import"), BUTTON_BG, ImportButton);
        button(row, font, &t("select.lang"), BUTTON_BG, LangButton);
        button(row, font, &t("help.button"), BUTTON_BG, HelpButton);
    });
    // Dev authoring controls (never in a ship build).
    #[cfg(feature = "dev")]
    root.spawn(Node {
        flex_direction: FlexDirection::Row,
        margin: UiRect::top(Val::Px(6.0)),
        ..default()
    })
    .with_children(|row| {
        button(row, font, "DEV: Fortschritt zurücksetzen", BUTTON_BG, DevResetProgress);
        button(row, font, "DEV: ALLE Level löschen", BUTTON_BG, DevDeleteAll);
    });
}

/// Chapter view: the levels of one chapter (with the global catalog index so
/// `click_level` is unchanged) plus a back button.
pub(super) fn build_chapter_view(
    root: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    catalog: &Catalog,
    progress: &Progress,
    chapter: u8,
) {
    root.spawn(text_bundle(font, chapter_name(chapter), 30.0, TEXT_BRIGHT));
    root.spawn(text_bundle(font, t("select.hint"), 14.0, TEXT_DIM));
    for (index, entry) in catalog
        .0
        .iter()
        .enumerate()
        .filter(|(_, e)| e.meta.chapter == chapter)
    {
        spawn_level_button(root, font, index, entry, progress);
    }
    root.spawn(Node {
        margin: UiRect::top(Val::Px(10.0)),
        ..default()
    })
    .with_children(|row| {
        button(row, font, &t("select.chapter_back"), BUTTON_BG, BackButton);
    });
}

/// A single level button: a solved dot, the name (+ optional-hard tag) and one
/// medal dot per score axis — all drawn icons, not font glyphs.
/// In a `dev` build the per-level delete sits beside it.
fn spawn_level_button(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    index: usize,
    entry: &LevelEntry,
    progress: &Progress,
) {
    let progress_entry = progress.levels.get(&entry.id);
    let medals = progress_entry
        .map(|p| p.medals(&entry.level))
        .unwrap_or_default();
    let solved = progress_entry.is_some_and(|p| p.solved);
    let mut label = level_name(&entry.id, &entry.level.name);
    if entry.meta.optional_hard {
        label.push_str("  ");
        label.push_str(&t("select.optional_hard"));
    }
    let fill = |r: &mut ChildSpawnerCommands| {
        if solved {
            dot(r, true, SOLVED);
        }
        r.spawn((
            text_bundle(font, label.clone(), 16.0, TEXT_BRIGHT),
            Node {
                margin: UiRect::horizontal(Val::Px(6.0)),
                ..default()
            },
        ));
        for achieved in &medals {
            dot(r, *achieved, MEDAL);
        }
    };
    #[cfg(feature = "dev")]
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|r| {
            button_row(r, BUTTON_BG, LevelButton(index), fill);
            small_button(r, font, "SBX", DevOpenSandbox(index));
            small_button(r, font, "DEL", DevDeleteLevel(entry.id.clone()));
        });
    #[cfg(not(feature = "dev"))]
    button_row(parent, BUTTON_BG, LevelButton(index), fill);
}

/// Chapter button → open that chapter's level view (rebuild in place).
#[allow(clippy::too_many_arguments)]
pub(super) fn chapter_clicks(
    interactions: Query<(&Interaction, &ChapterButton), Changed<Interaction>>,
    roots: Query<Entity, With<UiSelect>>,
    ui_font: Res<UiFont>,
    catalog: Res<Catalog>,
    progress: Res<Progress>,
    status: Res<UiStatus>,
    mut open: ResMut<OpenChapter>,
    mut commands: Commands,
) {
    for (interaction, btn) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        open.0 = Some(btn.0);
        rebuild_select(&mut commands, &roots, &ui_font.0, &catalog, &progress, open.0, &status.0);
        return;
    }
}

/// Back button → return to the chapter overview (rebuild in place).
#[allow(clippy::too_many_arguments)]
pub(super) fn back_click(
    interactions: Query<&Interaction, (Changed<Interaction>, With<BackButton>)>,
    roots: Query<Entity, With<UiSelect>>,
    ui_font: Res<UiFont>,
    catalog: Res<Catalog>,
    progress: Res<Progress>,
    status: Res<UiStatus>,
    mut open: ResMut<OpenChapter>,
    mut commands: Commands,
) {
    if interactions.iter().any(|i| *i == Interaction::Pressed) {
        open.0 = None;
        rebuild_select(&mut commands, &roots, &ui_font.0, &catalog, &progress, None, &status.0);
    }
}

pub(super) fn click_level(
    mut interactions: Query<(&Interaction, &LevelButton), Changed<Interaction>>,
    catalog: Res<Catalog>,
    progress: Res<Progress>,
    mut commands: Commands,
    mut editor: ResMut<Editor>,
    mut next: ResMut<NextState<GameState>>,
) {
    for (interaction, level_button) in &mut interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let entry = &catalog.0[level_button.0];
        enter_level(
            level_button.0,
            entry.id.clone(),
            entry.level.clone(),
            entry.meta.briefing.clone(),
            false,
            &progress,
            &mut commands,
            &mut editor,
            &mut next,
        );
    }
}

/// Dev: open a campaign level in the sandbox editor — same level, but its
/// definition (sources/sinks/schedule) becomes editable so it can be tweaked in
/// place instead of rebuilt from scratch. Exporting/saving from there writes the
/// level sim under its own id.
#[cfg(feature = "dev")]
pub(super) fn dev_open_sandbox(
    interactions: Query<(&Interaction, &DevOpenSandbox), Changed<Interaction>>,
    catalog: Res<Catalog>,
    progress: Res<Progress>,
    mut commands: Commands,
    mut editor: ResMut<Editor>,
    mut next: ResMut<NextState<GameState>>,
) {
    for (interaction, btn) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let entry = &catalog.0[btn.0];
        enter_level(
            btn.0,
            entry.id.clone(),
            entry.level.clone(),
            entry.meta.briefing.clone(),
            true,
            &progress,
            &mut commands,
            &mut editor,
            &mut next,
        );
    }
}
