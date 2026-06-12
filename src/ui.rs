//! bevy_ui screens: level select (with sandbox, code import, language
//! toggle), edit HUD (switch panel, solution slots, sandbox schedule
//! editor), run HUD, result overlay with code export (M2 plan §2).

use bevy::prelude::*;
use stellwerk_codes::Payload;
use stellwerk_sim::Outcome;
use stellwerk_sim::grid::Cell;
use stellwerk_sim::layout::{RuleWhen, SwitchDef, SwitchRule};
use stellwerk_sim::level::ScheduleEntry;
use stellwerk_sim::units::{Len, SinkId, Speed, Tick, TrainClass, TrainId};

use crate::editor::{EditOp, do_op};
use crate::i18n::{dir_label, set_lang, t};
use crate::levels::{Catalog, Progress, SANDBOX_ID, SOLUTION_SLOTS, load_sandbox, save_sandbox};
use crate::run::{RunCtl, TrainInfo};
use crate::state::{ActiveLevel, Diagnostics, Editor, GameState, LastOutcome, Tool};

#[derive(Component)]
struct UiSelect;
#[derive(Component)]
struct UiEdit;
#[derive(Component)]
struct UiRun;
#[derive(Component)]
struct UiResult;

#[derive(Component)]
struct LevelButton(usize);
#[derive(Component)]
struct SandboxButton;
#[derive(Component)]
struct ImportButton;
#[derive(Component)]
struct LangButton;
#[derive(Component)]
struct StatusText;
#[derive(Component)]
struct StartButton;
#[derive(Component)]
struct DiagText;
#[derive(Component)]
struct ToolText;
#[derive(Component)]
struct SpeedText;
#[derive(Component)]
struct InfoText;
#[derive(Component)]
struct SwitchPanelRoot;
#[derive(Component)]
struct SchedulePanelRoot;
#[derive(Component)]
struct ExportLevelButton;

/// Status line on the level select (import results etc.).
#[derive(Resource, Default)]
struct UiStatus(String);

/// Resting background color of a button — [`button_feedback`] lightens it on
/// hover/press and restores it on release.
#[derive(Component)]
struct ButtonBase(Color);

#[derive(Component, Clone, Copy)]
enum SlotAction {
    Save(usize),
    Load(usize),
}

#[derive(Component, Clone, Copy)]
enum PanelAction {
    ToggleDefault,
    CycleDest(SinkId),
    CycleClass(TrainClass),
    Close,
}

#[derive(Component, Clone, Copy)]
enum SchedAction {
    Add,
    Remove(usize),
    CycleSource(usize),
    CycleSink(usize),
    CycleClass(usize),
    BumpDepart(usize),
    BumpDue(usize),
    CycleSpeed(usize),
    CycleLength(usize),
}

#[derive(Component, Clone, Copy)]
enum ResultAction {
    BackEdit,
    NextLevel,
    LevelSelect,
    ExportCode,
}

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiStatus>()
            .add_systems(OnEnter(GameState::LevelSelect), spawn_select)
            .add_systems(OnExit(GameState::LevelSelect), despawn_all::<UiSelect>)
            .add_systems(
                Update,
                (click_level, select_buttons, update_status)
                    .run_if(in_state(GameState::LevelSelect)),
            )
            // Chained so the panel roots spawned by the HUD exist (deferred
            // commands flush between ordered systems) before the initial
            // fill — otherwise the panels stay empty when re-entering Edit
            // without an Editor/ActiveLevel change (e.g. back from Result).
            .add_systems(
                OnEnter(GameState::Edit),
                (spawn_edit_hud, rebuild_switch_panel, rebuild_schedule_panel).chain(),
            )
            .add_systems(OnExit(GameState::Edit), despawn_all::<UiEdit>)
            .add_systems(
                Update,
                (
                    update_edit_texts,
                    start_button,
                    rebuild_switch_panel.run_if(resource_changed::<Editor>),
                    rebuild_schedule_panel.run_if(resource_exists_and_changed::<ActiveLevel>),
                    panel_clicks,
                    slot_clicks,
                    schedule_clicks,
                    export_level_click,
                )
                    .run_if(in_state(GameState::Edit)),
            )
            .add_systems(OnEnter(GameState::Run), spawn_run_hud)
            .add_systems(OnExit(GameState::Result), despawn_all::<UiRun>)
            .add_systems(
                Update,
                update_run_texts.run_if(in_state(GameState::Run).or(in_state(GameState::Result))),
            )
            .add_systems(OnEnter(GameState::Result), spawn_result)
            .add_systems(OnExit(GameState::Result), despawn_all::<UiResult>)
            .add_systems(Update, result_clicks.run_if(in_state(GameState::Result)))
            // All states: hover/press feedback for every button.
            .add_systems(Update, button_feedback);
    }
}

/// Mixes a color toward white — visible feedback on the near-black theme.
fn lift(color: Color, amount: f32) -> Color {
    let c = color.to_srgba();
    Color::srgba(
        c.red + (1.0 - c.red) * amount,
        c.green + (1.0 - c.green) * amount,
        c.blue + (1.0 - c.blue) * amount,
        c.alpha,
    )
}

/// Buttons whose visual state needs a refresh this frame.
type ChangedButton = (With<Button>, Or<(Changed<Interaction>, Changed<ButtonBase>)>);

fn button_feedback(
    mut buttons: Query<(&Interaction, &ButtonBase, &mut BackgroundColor), ChangedButton>,
) {
    for (interaction, base, mut bg) in &mut buttons {
        let color = match interaction {
            Interaction::Pressed => lift(base.0, 0.25),
            Interaction::Hovered => lift(base.0, 0.10),
            Interaction::None => base.0,
        };
        *bg = BackgroundColor(color);
    }
}

fn despawn_all<C: Component>(mut commands: Commands, q: Query<Entity, With<C>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

/// Writes `value` only when it differs — these HUD texts are recomputed
/// every frame, and unconditional assignment forces a re-shape (and glyph
/// atlas churn) per frame even though nothing changed.
fn set_text(text: &mut Text, value: String) {
    if text.0 != value {
        text.0 = value;
    }
}

const PANEL_BG: Color = Color::srgba(0.05, 0.06, 0.08, 0.92);
const BUTTON_BG: Color = Color::srgb(0.10, 0.12, 0.16);
const BUTTON_BG_PRIMARY: Color = Color::srgb(0.10, 0.22, 0.14);
const BUTTON_BG_BLOCKED: Color = Color::srgb(0.22, 0.10, 0.10);
const TEXT_DIM: Color = Color::srgb(0.55, 0.58, 0.65);
const TEXT_BRIGHT: Color = Color::srgb(0.88, 0.90, 0.95);

fn text_bundle(value: String, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont::from_font_size(size),
        TextColor(color),
    )
}

fn button<M: Component>(parent: &mut ChildSpawnerCommands, label: &str, bg: Color, marker: M) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(7.0)),
                margin: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(bg),
            ButtonBase(bg),
            marker,
        ))
        .with_children(|b| {
            b.spawn(text_bundle(label.to_string(), 16.0, TEXT_BRIGHT));
        });
}

fn small_button<M: Component>(parent: &mut ChildSpawnerCommands, label: &str, marker: M) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)),
                margin: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(BUTTON_BG),
            ButtonBase(BUTTON_BG),
            marker,
        ))
        .with_children(|b| {
            b.spawn(text_bundle(label.to_string(), 13.0, TEXT_BRIGHT));
        });
}

// --- Level select -------------------------------------------------------------

fn spawn_select(mut commands: Commands, catalog: Res<Catalog>, progress: Res<Progress>) {
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
            root.spawn(text_bundle(t("select.title"), 30.0, TEXT_BRIGHT));
            root.spawn(text_bundle(t("select.hint"), 14.0, TEXT_DIM));
            for (index, entry) in catalog.0.iter().enumerate() {
                let progress_entry = progress.levels.get(&entry.id);
                let medals = progress_entry
                    .map(|p| p.medals(&entry.level))
                    .unwrap_or_default();
                let solved = progress_entry.is_some_and(|p| p.solved);
                let medal_str: String = medals.iter().map(|m| if *m { '●' } else { '○' }).collect();
                let check = if solved { "✓ " } else { "   " };
                button(
                    root,
                    &format!("{check}{}  {medal_str}", entry.level.name),
                    BUTTON_BG,
                    LevelButton(index),
                );
            }
            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                margin: UiRect::top(Val::Px(10.0)),
                ..default()
            })
            .with_children(|row| {
                button(row, &t("select.sandbox"), BUTTON_BG_PRIMARY, SandboxButton);
                button(row, &t("select.import"), BUTTON_BG, ImportButton);
                button(row, &t("select.lang"), BUTTON_BG, LangButton);
            });
            root.spawn((text_bundle(String::new(), 14.0, TEXT_DIM), StatusText));
        });
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
            true,
            &progress,
            &mut commands,
            &mut editor,
            &mut next,
        );
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
                Err(e) => status.0 = format!("{e}"),
                Ok(Payload::Solution { level_id, layout }) => {
                    if level_id == SANDBOX_ID || catalog.0.iter().any(|entry| entry.id == level_id)
                    {
                        progress.entry(&level_id).layout = layout;
                        progress.save();
                        status.0 = format!("Lösung importiert: {level_id}");
                    } else {
                        status.0 = format!("Unbekanntes Level: {level_id}");
                    }
                }
                Ok(Payload::Level { level }) => {
                    save_sandbox(&level);
                    status.0 = format!("Level importiert (Sandbox): {}", level.name);
                }
            },
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn enter_level(
    index: usize,
    id: String,
    level: stellwerk_sim::Level,
    sandbox: bool,
    progress: &Progress,
    commands: &mut Commands,
    editor: &mut Editor,
    next: &mut NextState<GameState>,
) {
    editor.layout = progress
        .levels
        .get(&id)
        .map(|p| p.layout.clone())
        .unwrap_or_default();
    editor.undo.clear();
    editor.redo.clear();
    editor.tool = Tool::Track;
    editor.variant = 0;
    editor.selected_switch = None;
    editor.drag = None;
    commands.insert_resource(ActiveLevel {
        id,
        index,
        level,
        sandbox,
    });
    next.set(GameState::Edit);
}

// --- Edit HUD -------------------------------------------------------------------

fn spawn_edit_hud(mut commands: Commands, active: Option<Res<ActiveLevel>>) {
    let (name, sandbox) = active
        .map(|a| (a.level.name.clone(), a.sandbox))
        .unwrap_or_default();
    // The container nodes carry `Interaction` so the board pointer can tell
    // "this click landed on UI" — also for clicks BETWEEN the buttons.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                top: Val::Px(8.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Interaction::default(),
            UiEdit,
        ))
        .with_children(|c| {
            c.spawn(text_bundle(name, 22.0, TEXT_BRIGHT));
            c.spawn((text_bundle(String::new(), 14.0, TEXT_DIM), ToolText));
            c.spawn(text_bundle(t("edit.hints"), 13.0, TEXT_DIM));
            c.spawn((
                text_bundle(String::new(), 14.0, Color::srgb(1.0, 0.45, 0.35)),
                DiagText,
            ));
            // Solution slots.
            c.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                row.spawn(text_bundle(t("edit.slots"), 13.0, TEXT_DIM));
                for i in 0..SOLUTION_SLOTS {
                    small_button(
                        row,
                        &format!("{}{}", t("edit.save_slot"), i + 1),
                        SlotAction::Save(i),
                    );
                }
                for i in 0..SOLUTION_SLOTS {
                    small_button(
                        row,
                        &format!("{}{}", t("edit.load_slot"), i + 1),
                        SlotAction::Load(i),
                    );
                }
            });
            if sandbox {
                c.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    ..default()
                })
                .with_children(|row| {
                    small_button(row, &t("edit.export_level"), ExportLevelButton);
                    small_button(row, &t("edit.add_train"), SchedAction::Add);
                });
            }
        });
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(10.0),
                top: Val::Px(8.0),
                ..default()
            },
            Interaction::default(),
            UiEdit,
        ))
        .with_children(|c| {
            button(c, &t("edit.start"), BUTTON_BG_PRIMARY, StartButton);
        });
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(10.0),
            top: Val::Px(64.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(Color::NONE),
        Interaction::default(),
        SwitchPanelRoot,
        UiEdit,
    ));
    // Always present: campaign levels show the timetable read-only — who
    // must go where is part of the puzzle statement, not something to
    // discover from a failed run. The sandbox panel is editable.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(10.0),
            bottom: Val::Px(10.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(PANEL_BG),
        Interaction::default(),
        SchedulePanelRoot,
        UiEdit,
    ));
}

fn update_edit_texts(
    editor: Res<Editor>,
    diagnostics: Res<Diagnostics>,
    active: Option<Res<ActiveLevel>>,
    mut tool_texts: Query<&mut Text, (With<ToolText>, Without<DiagText>)>,
    mut diag_texts: Query<&mut Text, (With<DiagText>, Without<ToolText>)>,
) {
    let sandbox = active.as_ref().is_some_and(|a| a.sandbox);
    if let Ok(mut text) = tool_texts.single_mut() {
        let tool = match editor.tool {
            Tool::Select => "Auswahl",
            Tool::Track => "Gleis",
            Tool::Switch => "Weiche",
            Tool::SignalBlock => "Blocksignal",
            Tool::SignalChain => "Kettensignal",
            Tool::Erase => "Abriss",
            Tool::Source => "Quelle",
            Tool::Sink => "Ziel",
        };
        let extra = if sandbox {
            t("edit.tools_sandbox")
        } else {
            String::new()
        };
        set_text(
            &mut text,
            format!("{}{extra}   |   Werkzeug: {tool}", t("edit.tools")),
        );
    }
    if let Ok(mut text) = diag_texts.single_mut() {
        let mut lines = Vec::new();
        for error in diagnostics.errors.iter().take(3) {
            lines.push(format!("✗ {error}"));
        }
        if diagnostics.errors.len() > 3 {
            lines.push(format!(
                "… +{} weitere Fehler",
                diagnostics.errors.len() - 3
            ));
        }
        for unreachable in diagnostics.unreachable.iter().take(2) {
            lines.push(format!("{}{}", t("edit.unreachable"), unreachable.train.0));
        }
        set_text(&mut text, lines.join("\n"));
    }
}

fn start_button(
    mut interactions: Query<(&Interaction, &mut ButtonBase, &Children), With<StartButton>>,
    mut texts: Query<&mut Text>,
    keys: Res<ButtonInput<KeyCode>>,
    diagnostics: Res<Diagnostics>,
    mut next: ResMut<NextState<GameState>>,
) {
    let allowed = diagnostics.start_allowed();
    let mut clicked = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space);
    for (interaction, mut base, children) in &mut interactions {
        let target = if allowed {
            BUTTON_BG_PRIMARY
        } else {
            BUTTON_BG_BLOCKED
        };
        // Write through ButtonBase (not BackgroundColor directly) so the
        // hover/press feedback keeps working — and only on actual change.
        if base.0 != target {
            base.0 = target;
        }
        if let Some(&child) = children.first()
            && let Ok(mut text) = texts.get_mut(child)
        {
            set_text(
                &mut text,
                if allowed {
                    t("edit.start")
                } else {
                    t("edit.start_blocked")
                },
            );
        }
        if *interaction == Interaction::Pressed {
            clicked = true;
        }
    }
    if clicked && allowed {
        next.set(GameState::Run);
    }
}

fn slot_clicks(
    mut interactions: Query<(&Interaction, &SlotAction), Changed<Interaction>>,
    active: Option<Res<ActiveLevel>>,
    mut editor: ResMut<Editor>,
    mut progress: ResMut<Progress>,
) {
    let Some(active) = active else { return };
    for (interaction, action) in &mut interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            SlotAction::Save(i) => {
                let layout = editor.layout.clone();
                progress.entry(&active.id).set_slot(*i, layout);
                progress.save();
            }
            SlotAction::Load(i) => {
                if let Some(layout) = progress
                    .levels
                    .get(&active.id)
                    .and_then(|p| p.slot(*i))
                    .cloned()
                {
                    editor.layout = layout;
                    editor.undo.clear();
                    editor.redo.clear();
                    editor.selected_switch = None;
                }
            }
        }
    }
}

fn export_level_click(
    interactions: Query<&Interaction, (Changed<Interaction>, With<ExportLevelButton>)>,
    active: Option<Res<ActiveLevel>>,
) {
    let Some(active) = active else { return };
    if interactions.iter().any(|i| *i == Interaction::Pressed) {
        let code = stellwerk_codes::encode(&Payload::Level {
            level: active.level.clone(),
        });
        if let Err(e) = std::fs::write("stellwerk_code.txt", code) {
            warn!("export failed: {e}");
        } else {
            info!("level code written to stellwerk_code.txt");
        }
        save_sandbox(&active.level);
    }
}

// --- Sandbox schedule editor ----------------------------------------------------

fn rebuild_schedule_panel(
    mut commands: Commands,
    roots: Query<Entity, With<SchedulePanelRoot>>,
    active: Option<Res<ActiveLevel>>,
) {
    let Ok(root) = roots.single() else { return };
    let Some(active) = active else { return };
    commands.entity(root).despawn_children();
    let level = active.level.clone();
    let sandbox = active.sandbox;
    let sink_label = |level: &stellwerk_sim::Level, id: SinkId| {
        level
            .sinks
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.label.clone())
            .unwrap_or_else(|| format!("Z{}", id.0))
    };
    commands.entity(root).with_children(|panel| {
        panel.spawn(text_bundle(t("schedule.title"), 15.0, TEXT_BRIGHT));
        if !sandbox {
            for entry in &level.schedule {
                panel.spawn(text_bundle(
                    format!(
                        "Zug {} · K{} · Q{} → {} · ab {} · soll {} · v{} · L{}",
                        entry.train.0,
                        entry.class.0,
                        entry.source.0,
                        sink_label(&level, entry.sink),
                        entry.depart.0,
                        entry.due.0,
                        entry.speed.0,
                        entry.length.0,
                    ),
                    13.0,
                    TEXT_DIM,
                ));
            }
            return;
        }
        for (row, entry) in level.schedule.iter().enumerate() {
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|r| {
                    r.spawn(text_bundle(
                        format!("Zug {}", entry.train.0),
                        13.0,
                        TEXT_DIM,
                    ));
                    small_button(
                        r,
                        &format!("Q{}", entry.source.0),
                        SchedAction::CycleSource(row),
                    );
                    small_button(
                        r,
                        &format!("→{}", sink_label(&level, entry.sink)),
                        SchedAction::CycleSink(row),
                    );
                    small_button(
                        r,
                        &format!("K{}", entry.class.0),
                        SchedAction::CycleClass(row),
                    );
                    small_button(
                        r,
                        &format!("ab {}", entry.depart.0),
                        SchedAction::BumpDepart(row),
                    );
                    small_button(
                        r,
                        &format!("soll {}", entry.due.0),
                        SchedAction::BumpDue(row),
                    );
                    small_button(
                        r,
                        &format!("v{}", entry.speed.0),
                        SchedAction::CycleSpeed(row),
                    );
                    small_button(
                        r,
                        &format!("L{}", entry.length.0),
                        SchedAction::CycleLength(row),
                    );
                    small_button(r, "×", SchedAction::Remove(row));
                });
        }
    });
}

fn schedule_clicks(
    mut interactions: Query<(&Interaction, &SchedAction), Changed<Interaction>>,
    active: Option<ResMut<ActiveLevel>>,
) {
    let Some(mut active) = active else { return };
    if !active.sandbox {
        return;
    }
    for (interaction, action) in &mut interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let level = &mut active.level;
        // Unknown current value (e.g. imported level) → start at the first
        // entry instead of silently skipping it.
        let cycle = |current: u32, list: &[u32]| -> u32 {
            let pos = list
                .iter()
                .position(|&v| v == current)
                .map_or(0, |p| (p + 1) % list.len());
            list[pos]
        };
        match *action {
            SchedAction::Add => {
                let (Some(source), Some(sink)) = (level.sources.first(), level.sinks.first())
                else {
                    continue; // needs at least one source and sink
                };
                let train = TrainId(
                    level
                        .schedule
                        .iter()
                        .map(|e| e.train.0)
                        .max()
                        .map_or(0, |m| m + 1),
                );
                let depart = Tick(level.schedule.last().map_or(0, |e| e.depart.0 + 10));
                level.schedule.push(ScheduleEntry {
                    train,
                    class: TrainClass(0),
                    length: Len(800),
                    speed: Speed(100),
                    source: source.id,
                    sink: sink.id,
                    depart,
                    due: Tick(depart.0 + 80),
                });
            }
            SchedAction::Remove(row) => {
                if row < level.schedule.len() {
                    level.schedule.remove(row);
                }
            }
            SchedAction::CycleSource(row) => {
                let ids: Vec<u32> = level.sources.iter().map(|s| s.id.0).collect();
                if let (Some(entry), false) = (level.schedule.get_mut(row), ids.is_empty()) {
                    entry.source = stellwerk_sim::units::SourceId(cycle(entry.source.0, &ids));
                }
            }
            SchedAction::CycleSink(row) => {
                let ids: Vec<u32> = level.sinks.iter().map(|s| s.id.0).collect();
                if let (Some(entry), false) = (level.schedule.get_mut(row), ids.is_empty()) {
                    entry.sink = SinkId(cycle(entry.sink.0, &ids));
                }
            }
            SchedAction::CycleClass(row) => {
                if let Some(entry) = level.schedule.get_mut(row) {
                    entry.class = TrainClass((entry.class.0 + 1) % 2);
                }
            }
            SchedAction::BumpDepart(row) => {
                if let Some(entry) = level.schedule.get_mut(row) {
                    entry.depart = Tick((entry.depart.0 + 10) % 200);
                    entry.due = Tick(entry.due.0.max(entry.depart.0 + 40));
                }
            }
            SchedAction::BumpDue(row) => {
                if let Some(entry) = level.schedule.get_mut(row) {
                    entry.due = Tick(entry.due.0 + 20);
                    if entry.due.0 > entry.depart.0 + 400 {
                        entry.due = Tick(entry.depart.0 + 40);
                    }
                }
            }
            SchedAction::CycleSpeed(row) => {
                if let Some(entry) = level.schedule.get_mut(row) {
                    entry.speed = Speed(cycle(entry.speed.0 as u32, &[60, 100, 150, 240]) as i64);
                }
            }
            SchedAction::CycleLength(row) => {
                if let Some(entry) = level.schedule.get_mut(row) {
                    entry.length = Len(cycle(entry.length.0 as u32, &[800, 1400, 1800]) as i64);
                }
            }
        }
    }
}

// --- Switch config panel ---------------------------------------------------------

/// Tri-state of a rule row: none / branch 0 / branch 1.
fn rule_state(switch: &SwitchDef, when: &RuleWhen) -> Option<u8> {
    switch
        .rules
        .iter()
        .find(|r| rule_matches(&r.when, when))
        .map(|r| r.branch)
}

fn rebuild_switch_panel(
    mut commands: Commands,
    roots: Query<Entity, With<SwitchPanelRoot>>,
    editor: Res<Editor>,
    active: Option<Res<ActiveLevel>>,
) {
    let Ok(root) = roots.single() else { return };
    let Some(active) = active else { return };
    commands.entity(root).despawn_children();

    let Some(cell) = editor.selected_switch else {
        commands.entity(root).insert(BackgroundColor(Color::NONE));
        return;
    };
    let Some(switch) = editor.layout.switches.iter().find(|s| s.cell == cell) else {
        return;
    };
    commands.entity(root).insert(BackgroundColor(PANEL_BG));

    let mut classes: Vec<TrainClass> = active.level.schedule.iter().map(|e| e.class).collect();
    classes.sort();
    classes.dedup();

    let switch = switch.clone();
    let sinks = active.level.sinks.clone();
    // Branches are named by their compass exit ("→ O"), not by index —
    // matches the labels drawn at the switch itself and follows rotation.
    let exit = |branch: u8| dir_label(switch.branches[branch as usize]);
    commands.entity(root).with_children(|panel| {
        panel.spawn(text_bundle(
            format!("{} ({}, {})", t("panel.switch_title"), cell.x, cell.y),
            18.0,
            TEXT_BRIGHT,
        ));
        button(
            panel,
            &format!("{}{}", t("panel.default"), exit(switch.default_branch)),
            BUTTON_BG,
            PanelAction::ToggleDefault,
        );
        panel.spawn(text_bundle(t("panel.rule_hint"), 12.0, TEXT_DIM));
        for sink in &sinks {
            let state = rule_state(&switch, &RuleWhen::DestIs(sink.id));
            let suffix = state.map_or(t("panel.rule_none"), |b| format!("→ {}", exit(b)));
            button(
                panel,
                &format!("{}{} {suffix}", t("panel.dest"), sink.label),
                BUTTON_BG,
                PanelAction::CycleDest(sink.id),
            );
        }
        for class in &classes {
            let state = rule_state(&switch, &RuleWhen::ClassIs(*class));
            let suffix = state.map_or(t("panel.rule_none"), |b| format!("→ {}", exit(b)));
            button(
                panel,
                &format!("{}{} {suffix}", t("panel.class"), class.0),
                BUTTON_BG,
                PanelAction::CycleClass(*class),
            );
        }
        button(panel, &t("panel.close"), BUTTON_BG, PanelAction::Close);
    });
}

fn panel_clicks(
    mut interactions: Query<(&Interaction, &PanelAction), Changed<Interaction>>,
    mut editor: ResMut<Editor>,
    active: Option<Res<ActiveLevel>>,
) {
    let Some(active) = active else { return };
    for (interaction, action) in &mut interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if matches!(action, PanelAction::Close) {
            editor.selected_switch = None;
            continue;
        }
        let Some(cell) = editor.selected_switch else {
            continue;
        };
        let Some(before) = editor
            .layout
            .switches
            .iter()
            .find(|s| s.cell == cell)
            .cloned()
        else {
            continue;
        };
        let mut after = before.clone();
        match action {
            PanelAction::ToggleDefault => after.default_branch = 1 - after.default_branch,
            PanelAction::CycleDest(sink) => cycle_rule(&mut after, RuleWhen::DestIs(*sink)),
            PanelAction::CycleClass(class) => cycle_rule(&mut after, RuleWhen::ClassIs(*class)),
            PanelAction::Close => unreachable!(),
        }
        normalize_rules(&mut after, &active.level);
        do_op(
            &mut editor,
            EditOp::Configure {
                cell,
                before,
                after,
            },
        );
    }
}

/// none → Zweig 0 → Zweig 1 → none.
fn cycle_rule(switch: &mut SwitchDef, when: RuleWhen) {
    match rule_state(switch, &when) {
        None => switch.rules.push(SwitchRule { when, branch: 0 }),
        Some(0) => {
            for rule in &mut switch.rules {
                if rule_matches(&rule.when, &when) {
                    rule.branch = 1;
                }
            }
        }
        Some(_) => switch.rules.retain(|r| !rule_matches(&r.when, &when)),
    }
}

fn rule_matches(a: &RuleWhen, b: &RuleWhen) -> bool {
    match (a, b) {
        (RuleWhen::DestIs(x), RuleWhen::DestIs(y)) => x == y,
        (RuleWhen::ClassIs(x), RuleWhen::ClassIs(y)) => x == y,
        _ => false,
    }
}

/// Panel-defined rule order: sink rows first, then class rows (M1-minimal —
/// free ordering comes with later editor polish).
fn normalize_rules(switch: &mut SwitchDef, level: &stellwerk_sim::Level) {
    let old = switch.rules.clone();
    let mut rules = Vec::new();
    for sink in &level.sinks {
        if let Some(rule) = old
            .iter()
            .find(|r| rule_matches(&r.when, &RuleWhen::DestIs(sink.id)))
        {
            rules.push(*rule);
        }
    }
    let mut classes: Vec<TrainClass> = level.schedule.iter().map(|e| e.class).collect();
    classes.sort();
    classes.dedup();
    for class in classes {
        if let Some(rule) = old
            .iter()
            .find(|r| rule_matches(&r.when, &RuleWhen::ClassIs(class)))
        {
            rules.push(*rule);
        }
    }
    switch.rules = rules;
}

// --- Run HUD ----------------------------------------------------------------------

fn spawn_run_hud(mut commands: Commands, active: Option<Res<ActiveLevel>>) {
    let name = active.map(|a| a.level.name.clone()).unwrap_or_default();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                top: Val::Px(8.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            UiRun,
        ))
        .with_children(|c| {
            c.spawn(text_bundle(name, 22.0, TEXT_BRIGHT));
            c.spawn((text_bundle(String::new(), 16.0, TEXT_BRIGHT), SpeedText));
            c.spawn(text_bundle(t("run.hints"), 13.0, TEXT_DIM));
            c.spawn((text_bundle(String::new(), 14.0, TEXT_DIM), InfoText));
        });
}

fn update_run_texts(
    ctl: Option<Res<RunCtl>>,
    info: Res<TrainInfo>,
    mut speed_texts: Query<&mut Text, (With<SpeedText>, Without<InfoText>)>,
    mut info_texts: Query<&mut Text, (With<InfoText>, Without<SpeedText>)>,
) {
    let Some(ctl) = ctl else { return };
    if let Ok(mut text) = speed_texts.single_mut() {
        let speed = if ctl.speed == 0 {
            t("run.paused")
        } else {
            format!("×{}", ctl.speed)
        };
        set_text(&mut text, format!("Tick {}   {speed}", ctl.sim.now().0));
    }
    if let Ok(mut text) = info_texts.single_mut() {
        set_text(&mut text, info.0.clone().unwrap_or_default());
    }
}

// --- Result overlay ------------------------------------------------------------------

fn spawn_result(
    mut commands: Commands,
    outcome: Option<Res<LastOutcome>>,
    active: Option<Res<ActiveLevel>>,
    catalog: Res<Catalog>,
) {
    let (Some(outcome), Some(active)) = (outcome, active) else {
        return;
    };
    let (headline, detail, color) = describe(&outcome.0);
    let success = matches!(outcome.0, Outcome::Success { .. });

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
            root.spawn(text_bundle(headline, 44.0, color));
            root.spawn(text_bundle(detail, 18.0, TEXT_BRIGHT));
            if let Outcome::Success { score } = &outcome.0 {
                let par = &active.level.par;
                let line = |name: String, value: u64, par_value: u64| {
                    let medal = if value <= par_value { '●' } else { '○' };
                    format!(
                        "{medal} {name}: {value}   ({}: {par_value})",
                        t("result.par")
                    )
                };
                root.spawn(text_bundle(
                    line(t("result.throughput"), score.throughput.0, par.throughput.0),
                    18.0,
                    TEXT_BRIGHT,
                ));
                root.spawn(text_bundle(
                    line(
                        t("result.material"),
                        score.material as u64,
                        par.material as u64,
                    ),
                    18.0,
                    TEXT_BRIGHT,
                ));
                root.spawn(text_bundle(
                    line(t("result.lateness"), score.lateness, par.lateness),
                    18.0,
                    TEXT_BRIGHT,
                ));
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
                    &t("result.back_edit"),
                    BUTTON_BG,
                    ResultAction::BackEdit,
                );
                if success && !active.sandbox && active.index + 1 < catalog.0.len() {
                    button(
                        row,
                        &t("result.next_level"),
                        BUTTON_BG_PRIMARY,
                        ResultAction::NextLevel,
                    );
                }
                button(
                    row,
                    &t("result.export"),
                    BUTTON_BG,
                    ResultAction::ExportCode,
                );
                button(
                    row,
                    &t("result.level_select"),
                    BUTTON_BG,
                    ResultAction::LevelSelect,
                );
            });
            root.spawn((text_bundle(String::new(), 14.0, TEXT_DIM), StatusText));
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
                "{}Zug {} und Zug {}.",
                t("result.collision_detail"),
                trains.0.0,
                trains.1.0
            ),
            Color::srgb(1.0, 0.3, 0.25),
        ),
        Outcome::Deadlock { cycle } => {
            let ids: Vec<String> = cycle.iter().map(|t| format!("Zug {}", t.0)).collect();
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
                format!("Zug {}: {what}{blame_str}", train.0),
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
                    let message = match std::fs::write("stellwerk_code.txt", code) {
                        Ok(()) => t("result.exported"),
                        Err(e) => format!("Export fehlgeschlagen: {e}"),
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
