//! Dev-only tool: "save the sandbox as a campaign level".
//! A small panel in the sandbox edit screen that writes the current sandbox
//! `sim` block out to `assets/levels/<id>.ron` with a filled `meta` block, plus
//! placeholder i18n keys — so the painful part (hand-writing buildable cells,
//! sources, sinks and the schedule) is done by a button.
//!
//! Chapter/order are typed into numeric fields (clamped to their valid ranges);
//! the id is generated (`k<chapter>_<order>_neu`, de-duplicated) and the
//! briefing starts empty — to be filled via the `i18n_fill` CLI and then
//! translated. Rename/refine in the file afterwards.

use bevy::prelude::*;
use stellwerk_sim::level::{LEVEL_SCHEMA_VERSION, LevelMeta};

use super::numeric_field::{NumericField, numeric_field, numeric_field_focus};
use super::widgets::{
    BUTTON_BG, BUTTON_BG_PRIMARY, PANEL_BG, TEXT_BRIGHT, TEXT_DIM, button, set_text, text_bundle,
};
use crate::console::ConsoleLog;
use crate::font::UiFont;
use crate::i18n::set_lang;
use crate::levels::{Catalog, Progress, SANDBOX_ID, load_catalog};
use crate::state::{ActiveLevel, Editor, GameState};

/// Chapter range matches the campaign; order steps of 10 leave room to insert.
const CHAPTER_MIN: i64 = 1;
const CHAPTER_MAX: i64 = 8;
const ORDER_MIN: i64 = 10;
const ORDER_MAX: i64 = 200;

/// Toggle inputs that don't come from the numeric fields. (The save result is
/// logged to the in-level console, not kept here.)
#[derive(Resource)]
struct CampaignDraft {
    hard: bool,
    /// Fold the on-screen build into `sim.fixed` on save. On = the build
    /// becomes pre-placed infrastructure (sandbox semantics, makes a level
    /// valid standalone); off = only the definition is written and `fixed` is
    /// left as authored, so a player-builds level isn't pre-solved. Reset to
    /// on each time the edit screen opens.
    fold: bool,
}

impl Default for CampaignDraft {
    fn default() -> Self {
        Self { hard: false, fold: true }
    }
}

#[derive(Component)]
struct UiCampaignSave;
#[derive(Component)]
struct ChapterField;
#[derive(Component)]
struct OrderField;
#[derive(Component)]
struct HardButton;
#[derive(Component)]
struct FoldButton;
#[derive(Component)]
struct SaveButton;
#[derive(Component)]
struct InfoText;

/// When this sandbox session was opened from a real campaign level (the SBX
/// dev button), saving overwrites THAT file with its original meta preserved,
/// instead of minting a fresh `_neu` level. `None` = a true sandbox.
fn overwrite_target(active: &ActiveLevel, catalog: &Catalog) -> Option<(String, LevelMeta)> {
    if active.id == SANDBOX_ID {
        return None;
    }
    catalog
        .0
        .iter()
        .find(|e| e.id == active.id)
        .map(|e| (e.id.clone(), e.meta.clone()))
}

pub(super) struct CampaignSavePlugin;

impl Plugin for CampaignSavePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CampaignDraft>()
            .add_systems(OnEnter(GameState::Edit), spawn_panel)
            .add_systems(OnExit(GameState::Edit), super::widgets::despawn_all::<UiCampaignSave>)
            .add_systems(
                Update,
                // `save_click` after `numeric_field_focus`: clicking Save blurs
                // the focused field, committing its typed buffer into `.value`.
                (
                    toggle_hard,
                    toggle_fold,
                    save_click.after(numeric_field_focus),
                    update_info,
                )
                    .run_if(in_state(GameState::Edit)),
            );
    }
}

fn spawn_panel(
    mut commands: Commands,
    ui_font: Res<UiFont>,
    active: Option<Res<ActiveLevel>>,
    mut draft: ResMut<CampaignDraft>,
) {
    // Only in the sandbox — campaign levels are not re-authored from inside.
    if !active.is_some_and(|a| a.sandbox) {
        return;
    }
    // Predictable defaults each time the screen opens.
    *draft = CampaignDraft::default();
    let font = ui_font.0.clone();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(10.0),
                bottom: Val::Px(10.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(PANEL_BG),
            Interaction::default(),
            UiCampaignSave,
        ))
        .with_children(|panel| {
            panel.spawn(text_bundle(
                &font,
                "DEV: Als Kampagnen-Level speichern".into(),
                14.0,
                TEXT_BRIGHT,
            ));
            panel.spawn((text_bundle(&font, String::new(), 13.0, TEXT_DIM), InfoText));
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(text_bundle(&font, "Kapitel".into(), 13.0, TEXT_DIM));
                    numeric_field(row, &font, CHAPTER_MIN, CHAPTER_MIN, CHAPTER_MAX, ChapterField);
                    row.spawn(text_bundle(&font, "Order".into(), 13.0, TEXT_DIM));
                    numeric_field(row, &font, ORDER_MIN, ORDER_MIN, ORDER_MAX, OrderField);
                    button(row, &font, "hart umschalten", BUTTON_BG, HardButton);
                });
            button(panel, &font, "Bau einbacken umschalten", BUTTON_BG, FoldButton);
            button(panel, &font, "Speichern", BUTTON_BG_PRIMARY, SaveButton);
        });
}

fn toggle_hard(
    hard: Query<&Interaction, (Changed<Interaction>, With<HardButton>)>,
    mut draft: ResMut<CampaignDraft>,
) {
    if hard.iter().any(|i| *i == Interaction::Pressed) {
        draft.hard = !draft.hard;
    }
}

fn toggle_fold(
    fold: Query<&Interaction, (Changed<Interaction>, With<FoldButton>)>,
    mut draft: ResMut<CampaignDraft>,
) {
    if fold.iter().any(|i| *i == Interaction::Pressed) {
        draft.fold = !draft.fold;
    }
}

#[allow(clippy::too_many_arguments)]
fn save_click(
    interactions: Query<&Interaction, (Changed<Interaction>, With<SaveButton>)>,
    chapter: Query<&NumericField, With<ChapterField>>,
    order: Query<&NumericField, With<OrderField>>,
    active: Option<Res<ActiveLevel>>,
    editor: Res<Editor>,
    mut progress: ResMut<Progress>,
    mut catalog: ResMut<Catalog>,
    draft: Res<CampaignDraft>,
    mut log: ResMut<ConsoleLog>,
) {
    if !interactions.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(active) = active.filter(|a| a.sandbox) else {
        return;
    };
    let (Ok(chapter), Ok(order)) = (chapter.single(), order.single()) else {
        return;
    };
    // Opened from a real campaign level (SBX) → overwrite that file, keeping its
    // original meta (chapter/order/briefing). A true sandbox → mint a `_neu`
    // level from the typed fields. Fields are clamped, so the casts never
    // truncate.
    let overwrite = overwrite_target(&active, &catalog);
    let (id, meta) = match &overwrite {
        Some((id, meta)) => (id.clone(), meta.clone()),
        None => {
            let chapter = chapter.value() as u8;
            let order = order.value() as u16;
            (
                unique_id(chapter, order),
                LevelMeta {
                    schema_version: LEVEL_SCHEMA_VERSION,
                    chapter,
                    order,
                    optional_hard: draft.hard,
                    briefing: String::new(),
                },
            )
        }
    };
    // "Bau einbacken": fold the on-screen build into `fixed` so it becomes
    // pre-placed infrastructure (the build's track anchors the sources/sinks).
    // Off → write only the definition and leave `fixed` as authored, so a
    // player-builds level isn't pre-solved. Forced on for an SBX overwrite: there
    // the layout IS the level's authored track (lifted out of `fixed` on open),
    // so not folding would drop it on save.
    let mut sim = active.level.clone();
    if draft.fold || overwrite.is_some() {
        sim.fixed = sim.fixed.merged(&editor.layout);
    }

    match crate::authoring::write_campaign_level(&id, meta, sim.clone()) {
        Ok(path) => {
            // i18n keys were appended; reload the live table, then refresh the
            // catalog so the new level shows up back in the level select.
            let lang = if progress.lang.is_empty() {
                "de"
            } else {
                &progress.lang
            };
            set_lang(lang);
            *catalog = load_catalog();
            // When the build was folded into `fixed` AND we overwrote the level
            // in place, the per-level autosave now duplicates that build — drop
            // it so re-opening via SBX doesn't stack a second copy on top.
            if overwrite.is_some() && draft.fold {
                progress.entry(&id).layout = stellwerk_sim::Layout::default();
                progress.save();
            }
            // Validate with an EMPTY player layout — exactly what
            // `tests/levels.rs` enforces. With the build folded in, an error
            // means the build itself is incomplete; without folding, it likely
            // means the authored `fixed` doesn't anchor the sources/sinks (the
            // player is meant to). Surface the first one to the console.
            let errors = stellwerk_sim::validate(&sim, &stellwerk_sim::Layout::default());
            if errors.is_empty() {
                log.info(format!("Gespeichert: {} · Katalog neu geladen", path.display()));
            } else {
                log.warn(format!(
                    "Gespeichert ({}), aber ungültig mit leerem Layout: {}",
                    path.display(),
                    errors[0]
                ));
            }
        }
        Err(e) => log.error(format!("Fehler beim Speichern: {e}")),
    }
}

fn update_info(
    draft: Res<CampaignDraft>,
    chapter: Query<&NumericField, With<ChapterField>>,
    order: Query<&NumericField, With<OrderField>>,
    active: Option<Res<ActiveLevel>>,
    catalog: Option<Res<Catalog>>,
    mut texts: Query<&mut Text, With<InfoText>>,
) {
    let (Ok(chapter), Ok(order)) = (chapter.single(), order.single()) else {
        return;
    };
    // Mirror `save_click`'s target choice so the line never lies about what
    // "Speichern" will do.
    let overwrite = match (active.as_ref(), catalog.as_ref()) {
        (Some(a), Some(c)) => overwrite_target(a, c),
        _ => None,
    };
    // SBX always folds (the layout is the level's authored track) — show that
    // rather than the raw toggle, so the line never lies about the result.
    let forced_fold = overwrite.is_some();
    let target = match overwrite {
        Some((id, _)) => format!("überschreibt {id}.ron"),
        None => format!("neu: {}.ron", preview_id(chapter.value() as u8, order.value() as u16)),
    };
    if let Ok(mut text) = texts.single_mut() {
        let hard = if draft.hard { "an" } else { "aus" };
        let fold = if forced_fold {
            "an (SBX)"
        } else if draft.fold {
            "an"
        } else {
            "aus"
        };
        set_text(
            &mut text,
            format!(
                "Kapitel: {} · Order: {} · hart: {hard} · einbacken: {fold} · {target}",
                chapter.value(),
                order.value()
            ),
        );
    }
}

/// id stem the next save would use, BEFORE de-duplication (preview only).
fn preview_id(chapter: u8, order: u16) -> String {
    format!("k{chapter}_{:02}_neu", order / 10)
}

/// A real, free file id: the preview stem, with `_2`, `_3`, … appended if a
/// file already exists. Falls back to the bare stem if everything is taken.
fn unique_id(chapter: u8, order: u16) -> String {
    let base = preview_id(chapter, order);
    let exists = |id: &str| std::path::Path::new("assets/levels").join(format!("{id}.ron")).exists();
    if !exists(&base) {
        return base;
    }
    (2..1000)
        .map(|n| format!("{base}_{n}"))
        .find(|cand| !exists(cand))
        .unwrap_or(base)
}
