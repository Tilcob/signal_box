//! Result overlay: outcome headline, score lines vs. par, navigation and
//! solution code export.

use bevy::prelude::*;
use stellwerk_codes::Payload;
use stellwerk_sim::Outcome;
use stellwerk_sim::grid::Cell;

use super::enter_level;
use super::widgets::{
    BUTTON_BG, BUTTON_BG_PRIMARY, MEDAL, StatusText, TEXT_BRIGHT, TEXT_DIM, button, despawn_all,
    dot, text_bundle,
};
use crate::clipboard::CopyOutcome;
use crate::console::{ConsoleLog, Severity};
use crate::font::UiFont;
use crate::i18n::t;
use crate::levels::{Catalog, Progress};
use crate::state::{ActiveLevel, Editor, GameState, LastOutcome};

#[derive(Component)]
struct UiResult;

#[derive(Component, Clone, Copy)]
enum ResultAction {
    BackEdit,
    NextLevel,
    LevelSelect,
    ExportCode,
}

/// Dev authoring: saves the just-succeeded build straight
/// into `solutions/<id>.ron` — no export/import detour.
#[cfg(feature = "dev")]
#[derive(Component)]
struct SaveSolutionButton;

pub(super) struct ResultPlugin;

impl Plugin for ResultPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Result), spawn_result)
            .add_systems(OnExit(GameState::Result), despawn_all::<UiResult>)
            .add_systems(Update, result_clicks.run_if(in_state(GameState::Result)));
        #[cfg(feature = "dev")]
        app.add_systems(
            Update,
            save_solution_click.run_if(in_state(GameState::Result)),
        );
    }
}

fn spawn_result(
    mut commands: Commands,
    ui_font: Res<UiFont>,
    outcome: Option<Res<LastOutcome>>,
    active: Option<Res<ActiveLevel>>,
    catalog: Res<Catalog>,
    mut log: ResMut<ConsoleLog>,
) {
    let (Some(outcome), Some(active)) = (outcome, active) else {
        return;
    };
    let font = ui_font.0.clone();
    let (headline, detail, color) = describe(&outcome.0);
    let success = matches!(outcome.0, Outcome::Success { .. });
    // Echo the outcome into the in-level console (success white, failure red).
    log.push(
        if success { Severity::Info } else { Severity::Error },
        format!("{headline} — {detail}"),
    );

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            UiResult,
        ))
        .with_children(|root| {
            root.spawn(text_bundle(&font, headline, 44.0, color));
            root.spawn(text_bundle(&font, detail, 18.0, TEXT_BRIGHT));
            if let Outcome::Success { score } = &outcome.0 {
                let par = &active.level.par;
                // Medal is a drawn dot (filled = at/under par), not a glyph —
                // the DIN UI font has no ●/○.
                let score_row = |root: &mut ChildSpawnerCommands,
                                     name: String,
                                     value: u64,
                                     par_value: u64| {
                    root.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|r| {
                        dot(r, value <= par_value, MEDAL);
                        r.spawn(text_bundle(
                            &font,
                            format!("{name}: {value}   ({}: {par_value})", t("result.par")),
                            18.0,
                            TEXT_BRIGHT,
                        ));
                    });
                };
                score_row(root, t("result.throughput"), score.throughput.0, par.throughput.0);
                score_row(root, t("result.material"), score.material as u64, par.material as u64);
                score_row(root, t("result.lateness"), score.lateness, par.lateness);
            }
            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                margin: UiRect::top(Val::Px(12.0)),
                ..default()
            })
            .with_children(|row| {
                button(
                    row,
                    &font,
                    &t("result.back_edit"),
                    BUTTON_BG,
                    ResultAction::BackEdit,
                );
                if success && !active.sandbox && active.index + 1 < catalog.0.len() {
                    button(
                        row,
                        &font,
                        &t("result.next_level"),
                        BUTTON_BG_PRIMARY,
                        ResultAction::NextLevel,
                    );
                }
                button(
                    row,
                    &font,
                    &t("result.export"),
                    BUTTON_BG,
                    ResultAction::ExportCode,
                );
                button(
                    row,
                    &font,
                    &t("result.level_select"),
                    BUTTON_BG,
                    ResultAction::LevelSelect,
                );
                // Dev authoring: stash the winning build as a designer solution.
                #[cfg(feature = "dev")]
                if success && !active.sandbox {
                    button(
                        row,
                        &font,
                        "DEV: Lösung sichern",
                        BUTTON_BG,
                        SaveSolutionButton,
                    );
                }
            });
            root.spawn((text_bundle(&font, String::new(), 14.0, TEXT_DIM), StatusText));
        });
}

fn describe(outcome: &Outcome) -> (String, String, Color) {
    match outcome {
        Outcome::Success { .. } => (
            t("result.success"),
            String::new(),
            Color::srgb(0.4, 1.0, 0.5),
        ),
        Outcome::Collision { trains, .. } => (
            t("result.collision"),
            format!(
                "{}{} {}, {} {}.",
                t("result.collision_detail"),
                t("common.train"),
                trains.0.0,
                t("common.train"),
                trains.1.0
            ),
            Color::srgb(1.0, 0.3, 0.25),
        ),
        Outcome::Deadlock { cycle } => {
            let ids: Vec<String> = cycle
                .iter()
                .map(|id| format!("{} {}", t("common.train"), id.0))
                .collect();
            (
                t("result.deadlock"),
                format!("{}{}.", t("result.deadlock_detail"), ids.join(" → ")),
                Color::srgb(1.0, 0.55, 0.2),
            )
        }
        Outcome::Misrouting {
            train,
            reached,
            blame,
        } => {
            let what = if reached.is_some() {
                t("result.misrouting_wrong")
            } else {
                t("result.misrouting_dead_end")
            };
            let blame_str = blame
                .map(|Cell { x, y }| format!(" {}({x}, {y}).", t("result.misrouting_blame")))
                .unwrap_or_default();
            (
                t("result.misrouting"),
                format!("{} {}: {what}{blame_str}", t("common.train"), train.0),
                Color::srgb(1.0, 0.7, 0.25),
            )
        }
        Outcome::Stalled { .. } => (
            t("result.stalled"),
            t("result.stalled_detail"),
            Color::srgb(0.8, 0.8, 0.4),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn result_clicks(
    mut interactions: Query<(&Interaction, &ResultAction), Changed<Interaction>>,
    catalog: Res<Catalog>,
    progress: Res<Progress>,
    active: Option<Res<ActiveLevel>>,
    mut status_texts: Query<&mut Text, With<StatusText>>,
    mut commands: Commands,
    mut editor: ResMut<Editor>,
    mut next: ResMut<NextState<GameState>>,
) {
    for (interaction, action) in &mut interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            ResultAction::BackEdit => next.set(GameState::Edit),
            ResultAction::LevelSelect => next.set(GameState::LevelSelect),
            ResultAction::ExportCode => {
                if let Some(active) = &active {
                    let code = stellwerk_codes::encode(&Payload::Solution {
                        level_id: active.id.clone(),
                        layout: editor.layout.clone(),
                    });
                    let message = match crate::clipboard::copy(&code) {
                        CopyOutcome::Clipboard => t("result.exported"),
                        CopyOutcome::File(_) => t("result.exported_file"),
                        CopyOutcome::Failed(e) => format!("{}: {e}", t("result.export_failed")),
                    };
                    if let Ok(mut text) = status_texts.single_mut() {
                        text.0 = message;
                    }
                }
            }
            ResultAction::NextLevel => {
                if let Some(active) = &active {
                    let index = active.index + 1;
                    if let Some(entry) = catalog.0.get(index) {
                        enter_level(
                            index,
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
            }
        }
    }
}

#[cfg(feature = "dev")]
fn save_solution_click(
    interactions: Query<&Interaction, (Changed<Interaction>, With<SaveSolutionButton>)>,
    active: Option<Res<ActiveLevel>>,
    editor: Res<Editor>,
    mut status_texts: Query<&mut Text, With<StatusText>>,
) {
    if !interactions.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(active) = active else { return };
    let msg = match crate::authoring::write_solution(&active.id, None, &editor.layout) {
        Ok(path) => format!("Designer-Lösung gesichert: {}", path.display()),
        Err(e) => format!("Sichern fehlgeschlagen: {e}"),
    };
    if let Ok(mut text) = status_texts.single_mut() {
        text.0 = msg;
    }
}
