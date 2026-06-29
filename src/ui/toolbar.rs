//! Left-edge tool rail for the edit HUD: one slot per tool (a composed glyph
//! plus its hotkey badge), the active tool highlighted, click selects. The
//! keyboard hotkeys stay the primary path — this is the visual/mouse mirror.
//!
//! Tool switches go through `bypass_change_detection` (exactly like the
//! hotkeys in `editor::tools`), so selecting a tool never marks `Editor`
//! changed and never rebuilds the board. The highlight therefore cannot gate
//! on `Editor::is_changed()`; it reconciles each slot's colour every frame
//! (guarded writes — only a real change touches `ButtonBase`).
//!
//! Glyphs are axis-aligned compositions of palette-coloured `Node`s (no
//! rotation: Bevy UI clips against the axis-aligned box, so a rotated shape
//! would lose its corners). Interim art, like `widgets::dot` — PNG icons can
//! replace them later.

use bevy::prelude::*;
use bevy::text::Font;

use super::edit_hud::UiEdit;
use super::widgets::{ButtonBase, PANEL_BG, TEXT_DIM, set_text, text_bundle};
use crate::font::UiFont;
use crate::state::{ActiveLevel, Editor, GameState, Tool};

/// Secondary marker on the rail container (the root carries `UiEdit`, so the
/// existing `despawn_all::<UiEdit>` on leaving Edit cleans the whole tree).
#[derive(Component)]
struct UiToolbar;

#[derive(Component, Clone, Copy)]
struct ToolSlot(Tool);

/// The single "R/T" rotate hint shown for rotatable tools.
#[derive(Component)]
struct RtBadge;

// Slot backgrounds (resting vs active). `ButtonBase` carries the resting
// colour; `widgets::button_feedback` lifts it on hover/press.
const SLOT_REST: Color = Color::srgb(0.10, 0.12, 0.16);
const SLOT_ACTIVE: Color = Color::srgb(0.16, 0.27, 0.20);

// Glyph accent colours — approximate evocations of the board, intentionally
// decoupled from `board::palette` (interim art).
const STEEL: Color = Color::srgb(0.45, 0.55, 0.70);
const SIGGREEN: Color = Color::srgb(0.35, 0.85, 0.50);
const RED: Color = Color::srgb(0.85, 0.35, 0.30);
const AMBER: Color = Color::srgb(0.80, 0.60, 0.30);
const SLATE: Color = Color::srgb(0.32, 0.30, 0.36);

pub(super) struct ToolbarPlugin;

impl Plugin for ToolbarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Edit), spawn_toolbar).add_systems(
            Update,
            // Chained so the highlight reconciles AFTER a click in the same
            // frame — no one-frame lag on mouse selection.
            (toolbar_click, toolbar_highlight, toolbar_rt_badge)
                .chain()
                .run_if(in_state(GameState::Edit)),
        );
        // Despawn rides on the edit HUD's `despawn_all::<UiEdit>` (the rail
        // root carries `UiEdit`).
    }
}

fn spawn_toolbar(mut commands: Commands, ui_font: Res<UiFont>, active: Option<Res<ActiveLevel>>) {
    let font = ui_font.0.clone();
    let sandbox = active.is_some_and(|a| a.sandbox);
    commands
        .spawn((
            // Full-height transparent shell, vertically centring the rail on
            // the left edge — clear of the top name panel and the bottom-left
            // timetable. No `Interaction`/background, so it never absorbs
            // board clicks along the strip; only the rail itself does.
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                ..default()
            },
            UiEdit,
        ))
        .with_children(|shell| {
            shell
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        padding: UiRect::all(Val::Px(4.0)),
                        row_gap: Val::Px(2.0),
                        ..default()
                    },
                    BackgroundColor(PANEL_BG),
                    Interaction::default(),
                    UiToolbar,
                ))
                .with_children(|rail| {
                    // Build tools.
                    slot(rail, &font, Tool::Track, "1");
                    slot(rail, &font, Tool::Switch, "2");
                    slot(rail, &font, Tool::SignalBlock, "3");
                    slot(rail, &font, Tool::SignalChain, "4");
                    separator(rail);
                    // Edit / inspect.
                    slot(rail, &font, Tool::Erase, "B");
                    slot(rail, &font, Tool::Select, "Q");
                    // Sandbox-only tools.
                    if sandbox {
                        separator(rail);
                        slot(rail, &font, Tool::Block, "5");
                        slot(rail, &font, Tool::Source, "6");
                        slot(rail, &font, Tool::Sink, "7");
                    }
                    rail.spawn((text_bundle(&font, String::new(), 11.0, TEXT_DIM), RtBadge));
                });
        });
}

fn separator(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Node {
            width: Val::Px(24.0),
            height: Val::Px(2.0),
            margin: UiRect::vertical(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(TEXT_DIM),
    ));
}

fn slot(parent: &mut ChildSpawnerCommands, font: &Handle<Font>, tool: Tool, key: &str) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(34.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(Val::Px(3.0)),
                row_gap: Val::Px(1.0),
                ..default()
            },
            BackgroundColor(SLOT_REST),
            ButtonBase(SLOT_REST),
            ToolSlot(tool),
        ))
        .with_children(|s| {
            s.spawn(Node {
                width: Val::Px(22.0),
                height: Val::Px(22.0),
                position_type: PositionType::Relative,
                ..default()
            })
            .with_children(|g| glyph(g, tool));
            s.spawn(text_bundle(font, key.to_string(), 10.0, TEXT_DIM));
        });
}

/// One axis-aligned rectangle inside a 22×22 glyph box.
fn bar(parent: &mut ChildSpawnerCommands, l: f32, t: f32, w: f32, h: f32, color: Color) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(l),
            top: Val::Px(t),
            width: Val::Px(w),
            height: Val::Px(h),
            ..default()
        },
        BackgroundColor(color),
    ));
}

fn glyph(g: &mut ChildSpawnerCommands, tool: Tool) {
    match tool {
        Tool::Track => bar(g, 2.0, 9.0, 18.0, 4.0, STEEL),
        Tool::Switch => {
            // A sideways fork: one track splits into two — reads as a turnout,
            // not a corner.
            bar(g, 2.0, 10.0, 9.0, 3.0, STEEL); // stem
            bar(g, 9.0, 5.0, 3.0, 12.0, STEEL); // junction riser
            bar(g, 11.0, 5.0, 7.0, 3.0, STEEL); // upper branch
            bar(g, 11.0, 14.0, 7.0, 3.0, STEEL); // lower branch
        }
        // The two signals must NOT look alike: square lamp on a post (block) vs.
        // round lamp on a post (chain). UI nodes can't rotate, so the chain's
        // board diamond becomes a circle here.
        Tool::SignalBlock => {
            bar(g, 7.0, 3.0, 8.0, 8.0, SIGGREEN); // square lamp
            bar(g, 10.0, 11.0, 2.0, 7.0, STEEL); // post
        }
        Tool::SignalChain => {
            g.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(7.0),
                    top: Val::Px(3.0),
                    width: Val::Px(8.0),
                    height: Val::Px(8.0),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(SIGGREEN), // round lamp
            ));
            bar(g, 10.0, 11.0, 2.0, 7.0, STEEL); // post
        }
        Tool::Erase => bar(g, 3.0, 9.0, 16.0, 4.0, RED),
        Tool::Source => {
            bar(g, 3.0, 7.0, 8.0, 8.0, STEEL);
            bar(g, 12.0, 9.0, 7.0, 4.0, STEEL);
        }
        Tool::Sink => {
            bar(g, 11.0, 7.0, 8.0, 8.0, AMBER);
            bar(g, 2.0, 9.0, 7.0, 4.0, AMBER);
        }
        Tool::Block => bar(g, 4.0, 4.0, 14.0, 14.0, SLATE),
        Tool::Select => {
            g.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(5.0),
                    top: Val::Px(5.0),
                    width: Val::Px(12.0),
                    height: Val::Px(12.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::all(TEXT_DIM),
            ));
        }
    }
}

/// Click a slot → select its tool via the bypass path (no board rebuild).
fn toolbar_click(
    slots: Query<(&Interaction, &ToolSlot), Changed<Interaction>>,
    mut editor: ResMut<Editor>,
) {
    for (interaction, slot) in &slots {
        if *interaction == Interaction::Pressed {
            editor.bypass_change_detection().tool = slot.0;
        }
    }
}

/// Reconcile each slot's resting colour to the active tool. Runs every frame
/// (the tool is bypass-set, so `Editor::is_changed` is unreliable); the write
/// is guarded, so only a real change touches `ButtonBase` and re-triggers
/// `button_feedback`.
fn toolbar_highlight(editor: Res<Editor>, mut slots: Query<(&ToolSlot, &mut ButtonBase)>) {
    for (slot, mut base) in &mut slots {
        let want = if slot.0 == editor.tool {
            SLOT_ACTIVE
        } else {
            SLOT_REST
        };
        if base.0 != want {
            base.0 = want;
        }
    }
}

/// Show the "R/T" rotate hint only for tools that have variants.
fn toolbar_rt_badge(editor: Res<Editor>, mut badges: Query<&mut Text, With<RtBadge>>) {
    let rotatable = matches!(
        editor.tool,
        Tool::Track | Tool::Switch | Tool::SignalBlock | Tool::SignalChain | Tool::Source | Tool::Sink
    );
    if let Ok(mut text) = badges.single_mut() {
        set_text(&mut text, if rotatable { "R/T".into() } else { String::new() });
    }
}
