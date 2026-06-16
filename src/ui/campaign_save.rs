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
use crate::font::UiFont;
use crate::i18n::set_lang;
use crate::levels::{Catalog, Progress, load_catalog};
use crate::state::{ActiveLevel, GameState};

/// Chapter range matches the campaign; order steps of 10 leave room to insert.
const CHAPTER_MIN: i64 = 1;
const CHAPTER_MAX: i64 = 8;
const ORDER_MIN: i64 = 10;
const ORDER_MAX: i64 = 200;

/// The save inputs that don't come from the numeric fields: the hard flag (a
/// toggle) and the last status line.
#[derive(Resource, Default)]
struct CampaignDraft {
    hard: bool,
    status: String,
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
struct SaveButton;
#[derive(Component)]
struct InfoText;

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
                (toggle_hard, save_click.after(numeric_field_focus), update_info)
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
    draft.status.clear();
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

#[allow(clippy::too_many_arguments)]
fn save_click(
    interactions: Query<&Interaction, (Changed<Interaction>, With<SaveButton>)>,
    chapter: Query<&NumericField, With<ChapterField>>,
    order: Query<&NumericField, With<OrderField>>,
    active: Option<Res<ActiveLevel>>,
    progress: Res<Progress>,
    mut catalog: ResMut<Catalog>,
    mut draft: ResMut<CampaignDraft>,
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
    // Fields are clamped to the ranges above, so the casts never truncate.
    let chapter = chapter.value as u8;
    let order = order.value as u16;
    let id = unique_id(chapter, order);
    let meta = LevelMeta {
        schema_version: LEVEL_SCHEMA_VERSION,
        chapter,
        order,
        optional_hard: draft.hard,
        briefing: String::new(),
    };
    match crate::authoring::write_campaign_level(&id, meta, active.level.clone()) {
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
            // A sandbox export has an EMPTY `fixed`; its sources/sinks are only
            // anchored on the (player) track you drew, so it usually fails
            // validate-with-empty-layout — exactly what `tests/levels.rs`
            // checks. Don't claim a clean save when the file won't pass: report
            // the first error so the author knows to add fixed anchor track.
            let errors = stellwerk_sim::validate(&active.level, &stellwerk_sim::Layout::default());
            draft.status = if errors.is_empty() {
                format!("Gespeichert: {} · Katalog neu geladen", path.display())
            } else {
                format!(
                    "Gespeichert ({}), ABER noch ungültig — fixed-Anker ergänzen: {}",
                    path.display(),
                    errors[0]
                )
            };
        }
        Err(e) => draft.status = format!("Fehler: {e}"),
    }
}

fn update_info(
    draft: Res<CampaignDraft>,
    chapter: Query<&NumericField, With<ChapterField>>,
    order: Query<&NumericField, With<OrderField>>,
    mut texts: Query<&mut Text, With<InfoText>>,
) {
    let (Ok(chapter), Ok(order)) = (chapter.single(), order.single()) else {
        return;
    };
    if let Ok(mut text) = texts.single_mut() {
        let id = preview_id(chapter.value as u8, order.value as u16);
        let hard = if draft.hard { "an" } else { "aus" };
        set_text(
            &mut text,
            format!(
                "Kapitel: {} · Order: {} · hart: {hard} · id≈{id}\n{}",
                chapter.value, order.value, draft.status
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
