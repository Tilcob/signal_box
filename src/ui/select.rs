//! Level select screen: catalog buttons with medals, sandbox entry, code
//! import and the language toggle.

use bevy::prelude::*;
use stellwerk_codes::{DecodeError, Payload};

use super::enter_level;
use super::encyclopedia::{HelpButton, HelpOpen};
#[cfg(feature = "dev")]
use super::widgets::small_button;
use super::widgets::{
    BUTTON_BG, BUTTON_BG_PRIMARY, StatusText, TEXT_BRIGHT, TEXT_DIM, button, despawn_all,
    set_text, text_bundle,
};
use crate::font::UiFont;
use crate::i18n::{level_name, set_lang, t, t_or};
use crate::levels::{Catalog, LevelEntry, Progress, SANDBOX_ID, load_sandbox, save_sandbox};
#[cfg(feature = "dev")]
use crate::levels::load_catalog;
use crate::state::{Editor, GameState};

#[derive(Component)]
struct UiSelect;
#[derive(Component)]
struct LevelButton(usize);
/// Opens the level view for a chapter (the campaign `chapter` number).
#[derive(Component)]
struct ChapterButton(u8);
/// Returns from a chapter's level view to the chapter overview.
#[derive(Component)]
struct BackButton;
#[derive(Component)]
struct SandboxButton;
#[derive(Component)]
struct NewSandboxButton;
#[derive(Component)]
struct ImportButton;
#[derive(Component)]
struct LangButton;

/// Status line content (import results etc.).
#[derive(Resource, Default)]
struct UiStatus(String);

/// Which chapter's level view is open. `None` = the chapter overview (the
/// screen's home), `Some(n)` = the level list of chapter `n`. Drives which of
/// the two layouts [`build_select`] builds; navigation rebuilds in place.
#[derive(Resource, Default)]
struct OpenChapter(Option<u8>);

// --- Dev authoring (optimierung/07): only built with feature `dev` ---------
/// Per-level delete (carries the level id, stable across re-indexing).
#[cfg(feature = "dev")]
#[derive(Component)]
struct DevDeleteLevel(String);
/// Wipe all progress (builds, slots, solved, scores) — keeps the language.
#[cfg(feature = "dev")]
#[derive(Component)]
struct DevResetProgress;
/// Delete EVERY level from disk — two-click armed (see [`DevDeleteArmed`]).
#[cfg(feature = "dev")]
#[derive(Component)]
struct DevDeleteAll;
/// Arms the destructive "delete all" so a single misclick cannot fire it.
#[cfg(feature = "dev")]
#[derive(Resource, Default)]
struct DevDeleteArmed(bool);

pub(super) struct SelectUiPlugin;

impl Plugin for SelectUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiStatus>()
            .init_resource::<OpenChapter>()
            .add_systems(
                OnEnter(GameState::LevelSelect),
                (spawn_select, update_status).chain(),
            )
            .add_systems(
                OnExit(GameState::LevelSelect),
                (despawn_all::<UiSelect>, reset_open_chapter),
            )
            .add_systems(
                Update,
                (
                    click_level,
                    select_buttons,
                    chapter_clicks,
                    back_click,
                    // Reads `HelpOpen` before the help overlay's Esc-close can
                    // flip it (see `encyclopedia::HelpEscClose`).
                    leave_to_menu.before(super::encyclopedia::HelpEscClose),
                )
                    .run_if(in_state(GameState::LevelSelect)),
            )
            // Status line changes only on explicit actions (import/lang) — no
            // need to rebuild + clone the string every frame.
            .add_systems(
                Update,
                update_status
                    .run_if(in_state(GameState::LevelSelect).and(resource_changed::<UiStatus>)),
            );
        #[cfg(feature = "dev")]
        app.init_resource::<DevDeleteArmed>().add_systems(
            Update,
            dev_select_actions.run_if(in_state(GameState::LevelSelect)),
        );
    }
}

/// Esc hierarchy on the level select: the help overlay owns Esc first (closes
/// itself); then a chapter's level view (back to the overview); only an Esc on
/// the bare overview leaves to the main menu.
#[allow(clippy::too_many_arguments)]
fn leave_to_menu(
    keys: Res<ButtonInput<KeyCode>>,
    help: Res<HelpOpen>,
    roots: Query<Entity, With<UiSelect>>,
    ui_font: Res<UiFont>,
    catalog: Res<Catalog>,
    progress: Res<Progress>,
    status: Res<UiStatus>,
    mut open: ResMut<OpenChapter>,
    mut commands: Commands,
    mut next: ResMut<NextState<GameState>>,
) {
    if help.0 || !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if open.0.is_some() {
        open.0 = None;
        rebuild_select(&mut commands, &roots, &ui_font.0, &catalog, &progress, None, &status.0);
    } else {
        next.set(GameState::MainMenu);
    }
}

/// Always land on the overview when (re)entering the screen.
fn reset_open_chapter(mut open: ResMut<OpenChapter>) {
    open.0 = None;
}

fn spawn_select(
    mut commands: Commands,
    ui_font: Res<UiFont>,
    catalog: Res<Catalog>,
    progress: Res<Progress>,
    open: Res<OpenChapter>,
    status: Res<UiStatus>,
) {
    build_select(&mut commands, &ui_font.0, &catalog, &progress, open.0, &status.0);
}

/// Despawns the current screen and rebuilds it in place. Used by chapter
/// navigation and the dev authoring actions so they need not bounce through
/// another state — the only way to refresh the screen mid-`LevelSelect`.
#[allow(clippy::too_many_arguments)]
fn rebuild_select(
    commands: &mut Commands,
    roots: &Query<Entity, With<UiSelect>>,
    font: &Handle<Font>,
    catalog: &Catalog,
    progress: &Progress,
    open_chapter: Option<u8>,
    status: &str,
) {
    for e in roots {
        commands.entity(e).despawn();
    }
    build_select(commands, font, catalog, progress, open_chapter, status);
}

/// Builds the level-select screen for the current view: the chapter overview
/// (`open_chapter == None`) or one chapter's level list. `status` is passed in
/// (not left to `update_status`) so an import/language message survives an
/// in-place rebuild — the rebuild path does not re-run `update_status`.
fn build_select(
    commands: &mut Commands,
    font: &Handle<Font>,
    catalog: &Catalog,
    progress: &Progress,
    open_chapter: Option<u8>,
    status: &str,
) {
    let font = font.clone();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(5.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            UiSelect,
        ))
        .with_children(|root| {
            match open_chapter {
                None => build_overview(root, &font, catalog, progress),
                Some(ch) => build_chapter_view(root, &font, catalog, progress, ch),
            }
            root.spawn((text_bundle(&font, status.to_string(), 14.0, TEXT_DIM), StatusText));
        });
}

/// Localized chapter name (authored, fallback "Kapitel N" — like level names,
/// so an unauthored chapter never breaks).
fn chapter_name(chapter: u8) -> String {
    t_or(&format!("chapter.{chapter}.name"), &format!("Kapitel {chapter}"))
}

/// Home view: one button per campaign chapter (with a solved/total summary),
/// plus the global sandbox/import/language/help row and dev controls.
fn build_overview(
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
        let label = format!("{}   {solved}/{total} ✓", chapter_name(ch));
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
fn build_chapter_view(
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

/// A single level button (label = medals/solved/hard), with the dev per-level
/// delete beside it in a `dev` build.
fn spawn_level_button(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    index: usize,
    entry: &LevelEntry,
    progress: &Progress,
) {
    let label = level_label(entry, progress);
    #[cfg(feature = "dev")]
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|r| {
            button(r, font, &label, BUTTON_BG, LevelButton(index));
            small_button(r, font, "DEL", DevDeleteLevel(entry.id.clone()));
        });
    #[cfg(not(feature = "dev"))]
    button(parent, font, &label, BUTTON_BG, LevelButton(index));
}

/// Level button label: solved check, name, medal dots, optional-hard tag.
fn level_label(entry: &LevelEntry, progress: &Progress) -> String {
    let progress_entry = progress.levels.get(&entry.id);
    let medals = progress_entry
        .map(|p| p.medals(&entry.level))
        .unwrap_or_default();
    let solved = progress_entry.is_some_and(|p| p.solved);
    let medal_str: String = medals.iter().map(|m| if *m { '●' } else { '○' }).collect();
    let check = if solved { "✓ " } else { "   " };
    let hard = if entry.meta.optional_hard {
        format!("  {}", t("select.optional_hard"))
    } else {
        String::new()
    };
    format!(
        "{check}{}  {medal_str}{hard}",
        level_name(&entry.id, &entry.level.name)
    )
}

/// Chapter button → open that chapter's level view (rebuild in place).
#[allow(clippy::too_many_arguments)]
fn chapter_clicks(
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
fn back_click(
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

fn update_status(status: Res<UiStatus>, mut texts: Query<&mut Text, With<StatusText>>) {
    if let Ok(mut text) = texts.single_mut() {
        set_text(&mut text, status.0.clone());
    }
}

fn click_level(
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

#[allow(clippy::too_many_arguments)]
fn select_buttons(
    sandbox: Query<&Interaction, (Changed<Interaction>, With<SandboxButton>)>,
    new_sandbox: Query<&Interaction, (Changed<Interaction>, With<NewSandboxButton>)>,
    import: Query<&Interaction, (Changed<Interaction>, With<ImportButton>)>,
    lang: Query<&Interaction, (Changed<Interaction>, With<LangButton>)>,
    catalog: Res<Catalog>,
    mut progress: ResMut<Progress>,
    mut status: ResMut<UiStatus>,
    mut commands: Commands,
    mut editor: ResMut<Editor>,
    mut next: ResMut<NextState<GameState>>,
) {
    if sandbox.iter().any(|i| *i == Interaction::Pressed) {
        let level = load_sandbox();
        enter_level(
            usize::MAX,
            SANDBOX_ID.to_string(),
            level,
            String::new(),
            true,
            &progress,
            &mut commands,
            &mut editor,
            &mut next,
        );
        return;
    }
    if new_sandbox.iter().any(|i| *i == Interaction::Pressed) {
        next.set(GameState::SandboxSetup);
        return;
    }
    if lang.iter().any(|i| *i == Interaction::Pressed) {
        let new_lang = if progress.lang == "en" { "de" } else { "en" };
        progress.lang = new_lang.to_string();
        progress.save();
        set_lang(new_lang);
        // Rebuild the screen with the new language.
        next.set(GameState::LevelSelect);
        status.0 = t("select.lang");
        return;
    }
    if import.iter().any(|i| *i == Interaction::Pressed) {
        match std::fs::read_to_string("stellwerk_import.txt") {
            Err(e) => status.0 = format!("stellwerk_import.txt: {e}"),
            Ok(text) => match stellwerk_codes::decode(&text) {
                Err(e) => status.0 = decode_error_text(&e),
                Ok(Payload::Solution { level_id, layout }) => {
                    if level_id == SANDBOX_ID || catalog.0.iter().any(|entry| entry.id == level_id)
                    {
                        progress.entry(&level_id).layout = layout;
                        progress.save();
                        status.0 = format!("{}{level_id}", t("select.import_ok"));
                    } else {
                        status.0 = format!("{}{level_id}", t("select.import_unknown"));
                    }
                }
                Ok(Payload::Level { level }) => {
                    save_sandbox(&level);
                    status.0 = format!("{}{}", t("select.import_sandbox"), level.name);
                }
            },
        }
    }
}

/// Every key [`decode_error_text`] can emit — kept beside the match so the
/// i18n coverage checker (see `crate::i18n` tests) asserts all of them resolve
/// in both languages. Adding a [`DecodeError`] variant breaks the exhaustive
/// match below and reminds you to extend this.
#[cfg(test)]
pub(crate) const DECODE_ERROR_KEYS: &[&str] = &[
    "import.error.prefix",
    "import.error.base64",
    "import.error.version",
    "import.error.corrupt",
];

/// Static chapter-navigation keys (the chapter names themselves use `t_or`
/// with an authored fallback, like level names, so they are not required here).
#[cfg(test)]
pub(crate) const SELECT_CHAPTER_KEYS: &[&str] = &["select.chapter_hint", "select.chapter_back"];

/// Localized import-failure text. `DecodeError`'s own `Display` stays English
/// (logs); the player-facing message is translated here — same split as
/// `edit_hud::valerr_text` for `ValidationError`.
pub(crate) fn decode_error_text(e: &DecodeError) -> String {
    match e {
        DecodeError::Prefix => t("import.error.prefix"),
        DecodeError::Base64 => t("import.error.base64"),
        DecodeError::Version(v) => format!("{} ({v})", t("import.error.version")),
        DecodeError::Corrupt => t("import.error.corrupt"),
    }
}

/// Dev authoring (optimierung/07): per-level delete, full progress reset, and a
/// two-click "delete all". After any mutation the catalog and i18n table are
/// reloaded and the screen is rebuilt in place. Only compiled with `dev`.
#[cfg(feature = "dev")]
#[allow(clippy::too_many_arguments)]
fn dev_select_actions(
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
