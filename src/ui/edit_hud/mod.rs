//! Edit HUD shell: level name, tool/hint/coord lines and the panel root nodes.
//! Submodules handle the rest — diagnostics→console ([`diag`]), the START
//! button ([`start`]) and the slot/export action buttons ([`actions`]).

mod actions;
mod diag;
mod start;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use stellwerk_sim::grid::Cell;

use super::schedule_panel::{SchedAction, SchedulePanelRoot, rebuild_schedule_panel};
use super::station_panel::{StationPanelRoot, rebuild_station_panel};
use super::switch_panel::{SwitchPanelRoot, rebuild_switch_panel};
use super::widgets::{
    PANEL_BG, TEXT_BRIGHT, TEXT_DIM, despawn_all, set_text, small_button, text_bundle,
};
use crate::font::UiFont;
use crate::i18n::{level_name, t};
use crate::state::{ActiveLevel, Editor, FocusedField, GameState, Tool};

/// Marker for everything despawned when leaving Edit (incl. panel roots).
/// `pub(crate)` so the edit-HUD submodules (`diag`/`start`/`actions`) and the
/// sibling `toolbar` can all tag their roots with it.
#[derive(Component)]
pub(crate) struct UiEdit;

#[derive(Component)]
struct ToolText;
/// Live cell coordinate under the cursor — a building aid for hand-editing the
/// `.ron` (fixed track, source/sink cells) without eyeballing the grid.
#[derive(Component)]
struct CoordsText;

pub(super) struct EditHudPlugin;

impl Plugin for EditHudPlugin {
    fn build(&self, app: &mut App) {
        // Chained so the panel roots spawned by the HUD exist (deferred
        // commands flush between ordered systems) before the initial
        // fill — otherwise the panels stay empty when re-entering Edit
        // without an Editor/ActiveLevel change (e.g. back from Result).
        app.add_plugins((diag::DiagPlugin, start::StartPlugin, actions::ActionsPlugin))
            .add_systems(
            OnEnter(GameState::Edit),
            (
                spawn_edit_hud,
                rebuild_switch_panel,
                rebuild_schedule_panel,
                rebuild_station_panel,
                fill_edit_texts,
            )
                .chain(),
        )
        .add_systems(OnExit(GameState::Edit), despawn_all::<UiEdit>)
        .add_systems(
            Update,
            (update_edit_texts, update_coords).run_if(in_state(GameState::Edit)),
        );
    }
}

fn spawn_edit_hud(
    mut commands: Commands,
    ui_font: Res<UiFont>,
    active: Option<Res<ActiveLevel>>,
) {
    let font = ui_font.0.clone();
    let (name, brief, sandbox) = active
        .map(|a| {
            (
                level_name(&a.id, &a.level.name),
                crate::i18n::briefing(&a.id, &a.briefing),
                a.sandbox,
            )
        })
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
                // Wrap the briefing before it reaches the top-right START button.
                // ponytail: percent of window width; clears a ~250px button down
                // to ~900px windows. Subtract the button's px width if narrower
                // windows ever overlap.
                max_width: Val::Percent(68.0),
                ..default()
            },
            Interaction::default(),
            UiEdit,
        ))
        .with_children(|c| {
            c.spawn(text_bundle(&font, name, 22.0, TEXT_BRIGHT));
            // Operating order: the puzzle statement, shown right on
            // the desk. Campaign levels carry one; the sandbox does not.
            if !brief.is_empty() {
                c.spawn(text_bundle(&font, brief, 14.0, TEXT_BRIGHT));
            }
            // The tool status line is superseded by the left tool rail
            // (`ui::toolbar`). Kept in code — node, `tool_line`, `tool_key` and
            // the update system stay — but hidden via `Display::None` (no
            // layout gap; trivially re-enabled by dropping this Node).
            c.spawn((
                text_bundle(&font, String::new(), 14.0, TEXT_DIM),
                ToolText,
                Node {
                    display: Display::None,
                    ..default()
                },
            ));
            c.spawn(text_bundle(&font, t("edit.hints"), 13.0, TEXT_DIM));
            c.spawn((text_bundle(&font, String::new(), 13.0, TEXT_DIM), CoordsText));
            // Diagnostics no longer live here — they go to the console
            // (`diag::mirror_diagnostics_to_console`), clickable to recentre.
            actions::slots_row(c, &font);
            if sandbox {
                c.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    ..default()
                })
                .with_children(|row| {
                    actions::export_button(row, &font);
                    small_button(row, &font, &t("edit.add_train"), SchedAction::Add);
                });
            }
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
    // Bottom-left group: the timetable and (sandbox) the station-rename panel sit
    // side by side in one flex Row so they can never overlap — and so the
    // station panel no longer collides with the dev save panel bottom-right.
    // Only the wrapper carries `UiEdit` (despawn is recursive); each child keeps
    // its own `Interaction` so board clicks are absorbed (`over_ui` reads every
    // `Interaction` flat, regardless of hierarchy).
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                bottom: Val::Px(10.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexEnd,
                column_gap: Val::Px(10.0),
                ..default()
            },
            UiEdit,
        ))
        .with_children(|group| {
            // Always present: campaign levels show the timetable read-only — who
            // must go where is part of the puzzle statement, not something to
            // discover from a failed run. The sandbox panel is editable.
            group.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
                Interaction::default(),
                SchedulePanelRoot,
            ));
            // Sandbox-only content; collapses to `Display::None` in campaign so it
            // leaves no phantom padded box beside the timetable (see
            // `rebuild_station_panel`).
            group.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
                Interaction::default(),
                StationPanelRoot,
            ));
        });
}

/// Tool/sandbox status line.
fn tool_line(tool: Tool, sandbox: bool) -> String {
    let extra = if sandbox {
        t("edit.tools_sandbox")
    } else {
        String::new()
    };
    format!(
        "{}{extra}   |   {}{}",
        t("edit.tools"),
        t("edit.tool_label"),
        t(tool_key(tool))
    )
}

/// One-shot fill on entering Edit: the HUD text entities are respawned on every
/// entry (incl. Result → "back to edit", which changes no resource), so the
/// guarded per-frame updater alone would leave them empty.
fn fill_edit_texts(
    editor: Res<Editor>,
    active: Option<Res<ActiveLevel>>,
    mut focus: ResMut<FocusedField>,
    mut tool_texts: Query<&mut Text, With<ToolText>>,
) {
    // Drop any stale focus from a previous Edit session (the fields are respawned
    // on entry). The diagnostics diff set is reset separately by `diag`.
    focus.0 = None;
    let sandbox = active.as_ref().is_some_and(|a| a.sandbox);
    if let Ok(mut text) = tool_texts.single_mut() {
        set_text(&mut text, tool_line(editor.tool, sandbox));
    }
}

/// Per-frame refresh of the tool line, rebuilt only when the tool/sandbox flag
/// changes — idle frames allocate nothing. The tool is switched via
/// `bypass_change_detection` (so it doesn't trigger the board rebuild), hence a
/// `Local` compare instead of `editor.is_changed()`. Diagnostics are handled by
/// `rebuild_diag_panel` (gated on `Diagnostics` change).
fn update_edit_texts(
    editor: Res<Editor>,
    active: Option<Res<ActiveLevel>>,
    mut tool_texts: Query<&mut Text, With<ToolText>>,
    mut last_tool: Local<Option<(Tool, bool)>>,
) {
    let sandbox = active.as_ref().is_some_and(|a| a.sandbox);
    if *last_tool != Some((editor.tool, sandbox)) {
        *last_tool = Some((editor.tool, sandbox));
        if let Ok(mut text) = tool_texts.single_mut() {
            set_text(&mut text, tool_line(editor.tool, sandbox));
        }
    }
}

/// Shows the cell under the cursor, refreshed only when it changes (no
/// per-frame allocation while the mouse is still). Blank when off the board.
fn update_coords(
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<crate::camera::MainCamera>>,
    mut texts: Query<&mut Text, With<CoordsText>>,
    mut last: Local<Option<Cell>>,
) {
    let cell = crate::camera::cursor_world(&windows, &cameras).map(crate::board::world_cell);
    if *last == cell {
        return;
    }
    *last = cell;
    if let Ok(mut text) = texts.single_mut() {
        set_text(
            &mut text,
            cell.map_or(String::new(), |c| format!("({}, {})", c.x, c.y)),
        );
    }
}

/// i18n key for a tool's display name. Exhaustive — a new [`Tool`] variant
/// will not compile until it gets a key here.
pub(crate) fn tool_key(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "tool.select",
        Tool::Track => "tool.track",
        Tool::Switch => "tool.switch",
        Tool::SignalBlock => "tool.signal_block",
        Tool::SignalChain => "tool.signal_chain",
        Tool::Erase => "tool.erase",
        Tool::Source => "tool.source",
        Tool::Sink => "tool.sink",
        Tool::Block => "tool.block",
    }
}
