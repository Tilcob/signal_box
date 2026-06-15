//! Switch config panel (right side): default exit and per-sink/per-class
//! routing rules, named by compass exit.

use bevy::prelude::*;
use stellwerk_sim::layout::{RuleWhen, SwitchDef, SwitchRule};
use stellwerk_sim::units::{SinkId, TrainClass};

use super::widgets::{BUTTON_BG, PANEL_BG, TEXT_BRIGHT, TEXT_DIM, button, small_button, text_bundle};
use crate::editor::{EditOp, do_op};
use crate::font::UiFont;
use crate::i18n::{dir_label, station_label, t};
use crate::state::{ActiveLevel, Editor, GameState};

/// Root node, spawned by the edit HUD (it owns the Edit screen layout).
#[derive(Component)]
pub(super) struct SwitchPanelRoot;

#[derive(Component, Clone, Copy)]
enum PanelAction {
    ToggleDefault,
    CycleDest(SinkId),
    CycleClass(TrainClass),
    Close,
}

pub(super) struct SwitchPanelPlugin;

impl Plugin for SwitchPanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                rebuild_switch_panel.run_if(resource_changed::<Editor>),
                panel_clicks,
            )
                .run_if(in_state(GameState::Edit)),
        );
    }
}

/// Tri-state of a rule row: none / branch 0 / branch 1.
fn rule_state(switch: &SwitchDef, when: &RuleWhen) -> Option<u8> {
    switch
        .rules
        .iter()
        .find(|r| rule_matches(&r.when, when))
        .map(|r| r.branch)
}

pub(super) fn rebuild_switch_panel(
    mut commands: Commands,
    ui_font: Res<UiFont>,
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

    let font = ui_font.0.clone();
    let switch = switch.clone();
    // Only offer destinations that lie behind the switch (reachable through a
    // branch). Sinks before it can never be routed here, so listing them only
    // confuses. On an invalid layout (graph won't build) fall back to all.
    let reachable =
        stellwerk_sim::routing::reachable_sinks(&active.level, &editor.layout, cell).ok().flatten();
    let sinks: Vec<_> = active
        .level
        .sinks
        .iter()
        .filter(|s| reachable.as_ref().is_none_or(|set| set.contains(&s.id)))
        .cloned()
        .collect();
    // Branches are named by their compass exit ("→ O"), not by index —
    // matches the labels drawn at the switch itself and follows rotation.
    let exit = |branch: u8| dir_label(switch.branches[branch as usize]);
    commands.entity(root).with_children(|panel| {
        panel.spawn(text_bundle(
            &font,
            format!("{} ({}, {})", t("panel.switch_title"), cell.x, cell.y),
            18.0,
            TEXT_BRIGHT,
        ));
        button(
            panel,
            &font,
            &format!("{}{}", t("panel.default"), exit(switch.default_branch)),
            BUTTON_BG,
            PanelAction::ToggleDefault,
        );
        for sink in &sinks {
            let state = rule_state(&switch, &RuleWhen::DestIs(sink.id));
            let suffix = state.map_or(t("panel.rule_none"), |b| format!("→ {}", exit(b)));
            button(
                panel,
                &font,
                &format!("{}{} {suffix}", t("panel.dest"), station_label(&sink.label)),
                BUTTON_BG,
                PanelAction::CycleDest(sink.id),
            );
        }
        for class in &classes {
            let state = rule_state(&switch, &RuleWhen::ClassIs(*class));
            let suffix = state.map_or(t("panel.rule_none"), |b| format!("→ {}", exit(b)));
            button(
                panel,
                &font,
                &format!("{}{} {suffix}", t("panel.class"), class.0),
                BUTTON_BG,
                PanelAction::CycleClass(*class),
            );
        }
        // Explainer below the rule buttons, with breathing room — directly
        // attached it reads as part of the buttons and looks cramped.
        panel.spawn((
            text_bundle(&font, t("panel.rule_hint"), 11.0, TEXT_DIM),
            Node {
                margin: UiRect::vertical(Val::Px(8.0)),
                ..default()
            },
        ));
        small_button(panel, &font, &t("panel.close"), PanelAction::Close);
    });
}

fn panel_clicks(
    mut interactions: Query<(&Interaction, &PanelAction), Changed<Interaction>>,
    mut editor: ResMut<Editor>,
    active: Option<ResMut<ActiveLevel>>,
    mut commands: Commands,
) {
    let Some(mut active) = active else { return };
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
            &mut active.level,
            EditOp::Configure {
                cell,
                before,
                after,
            },
        );
        // Throwing the switch (default exit) or changing a routing rule: the
        // physical switch-track sound. The global button-click still fires too.
        commands.trigger(crate::audio::SfxKind::Switch);
    }
}

/// none → branch 0 → branch 1 → none.
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
