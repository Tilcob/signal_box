//! Tool 1 of optimierung/07 (dev only): "save the sandbox as a campaign level".
//! A small panel in the sandbox edit screen that writes the current sandbox
//! `sim` block out to `assets/levels/<id>.ron` with a filled `meta` block, plus
//! placeholder i18n keys — so the painful part (hand-writing buildable cells,
//! sources, sinks and the schedule) is done by a button.
//!
//! Deliberately no free-text entry (the engine has no text field and a
//! hand-rolled one is fragile): chapter/order are cycle buttons, the id is
//! generated (`k<chapter>_<order>_neu`, de-duplicated) and the briefing starts
//! empty — to be filled via the `i18n_fill` CLI and then translated. This is
//! the pragmatic path the plan sanctions; rename/refine in the file afterwards.

use bevy::prelude::*;
use stellwerk_sim::level::{LEVEL_SCHEMA_VERSION, LevelMeta};

use super::widgets::{
    BUTTON_BG, BUTTON_BG_PRIMARY, PANEL_BG, TEXT_BRIGHT, TEXT_DIM, button, set_text, text_bundle,
};
use crate::font::UiFont;
use crate::i18n::set_lang;
use crate::levels::{Catalog, Progress, load_catalog};
use crate::state::{ActiveLevel, GameState};

/// Draft metadata for the next save (chapter/order picked via buttons) plus the
/// last status line.
#[derive(Resource)]
struct CampaignDraft {
    chapter: u8,
    order: u16,
    status: String,
}

impl Default for CampaignDraft {
    fn default() -> Self {
        CampaignDraft {
            chapter: 1,
            order: 10,
            status: String::new(),
        }
    }
}

#[derive(Component)]
struct UiCampaignSave;
#[derive(Component)]
struct ChapterButton;
#[derive(Component)]
struct OrderButton;
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
                (cycle_buttons, save_click, update_info).run_if(in_state(GameState::Edit)),
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
                    ..default()
                })
                .with_children(|row| {
                    button(row, &font, "Kapitel +", BUTTON_BG, ChapterButton);
                    button(row, &font, "Order +10", BUTTON_BG, OrderButton);
                });
            button(panel, &font, "Speichern", BUTTON_BG_PRIMARY, SaveButton);
        });
}

fn cycle_buttons(
    chapter: Query<&Interaction, (Changed<Interaction>, With<ChapterButton>)>,
    order: Query<&Interaction, (Changed<Interaction>, With<OrderButton>)>,
    mut draft: ResMut<CampaignDraft>,
) {
    if chapter.iter().any(|i| *i == Interaction::Pressed) {
        // 1..=8 wrap — matches the campaign's chapter range.
        draft.chapter = draft.chapter % 8 + 1;
    }
    if order.iter().any(|i| *i == Interaction::Pressed) {
        // 10, 20, … 200, wrap. Steps of 10 leave room to insert later.
        draft.order = if draft.order >= 200 { 10 } else { draft.order + 10 };
    }
}

#[allow(clippy::too_many_arguments)]
fn save_click(
    interactions: Query<&Interaction, (Changed<Interaction>, With<SaveButton>)>,
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
    let id = unique_id(draft.chapter, draft.order);
    let meta = LevelMeta {
        schema_version: LEVEL_SCHEMA_VERSION,
        chapter: draft.chapter,
        order: draft.order,
        optional_hard: false,
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
            draft.status = format!("Gespeichert: {} · Katalog neu geladen", path.display());
        }
        Err(e) => draft.status = format!("Fehler: {e}"),
    }
}

fn update_info(draft: Res<CampaignDraft>, mut texts: Query<&mut Text, With<InfoText>>) {
    if let Ok(mut text) = texts.single_mut() {
        let id = preview_id(draft.chapter, draft.order);
        set_text(
            &mut text,
            format!(
                "Kapitel: {} · Order: {} · id≈{id}\n{}",
                draft.chapter, draft.order, draft.status
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
