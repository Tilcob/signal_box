//! HUD (score + controls hint), pause indicator, end-of-run overlay, restart.

use bevy::prelude::*;

use crate::core::{GameState, RunState};
use crate::sim::{self, EndCause, Score, Signal, Timetable, TrackGraph, Train};

#[derive(Component)]
struct ScoreText;

#[derive(Component)]
struct EndOverlay;

#[derive(Component)]
struct PauseOverlay;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud)
            .add_systems(Update, (update_score_text, restart_on_r))
            .add_systems(OnEnter(GameState::Ended), spawn_end_overlay)
            .add_systems(OnExit(GameState::Ended), despawn_all::<EndOverlay>)
            .add_systems(OnEnter(RunState::Paused), spawn_pause_overlay)
            .add_systems(OnExit(RunState::Paused), despawn_all::<PauseOverlay>);
    }
}

fn spawn_hud(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont::from_font_size(20.0),
        TextColor(Color::srgb(0.9, 0.9, 0.92)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            top: Val::Px(8.0),
            ..default()
        },
        ScoreText,
        Name::new("ScoreText"),
    ));
    commands.spawn((
        Text::new(
            "Klick auf Signal: rot/grün halten · Klick auf Weiche: Abzweig wechseln · \
             Esc: Pause · R: Neustart",
        ),
        TextFont::from_font_size(14.0),
        TextColor(Color::srgb(0.55, 0.55, 0.6)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            bottom: Val::Px(8.0),
            ..default()
        },
        Name::new("ControlsHint"),
    ));
}

fn update_score_text(
    score: Res<Score>,
    timetable: Res<Timetable>,
    mut texts: Query<&mut Text, With<ScoreText>>,
) {
    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    let total = timetable.entries.len();
    text.0 = format!(
        "Zugestellt: {}/{total}   Fehlgeleitet: {}   Ausstehende Abfahrten: {}",
        score.delivered,
        score.misrouted,
        timetable.remaining(),
    );
}

fn spawn_end_overlay(
    mut commands: Commands,
    cause: Option<Res<EndCause>>,
    score: Res<Score>,
    timetable: Res<Timetable>,
) {
    let total = timetable.entries.len() as u32;
    let (headline, detail, color) = match cause.as_deref() {
        Some(EndCause::Crash) => (
            "KOLLISION!".to_string(),
            "Zwei Züge sind zusammengestoßen.".to_string(),
            Color::srgb(0.95, 0.25, 0.25),
        ),
        _ => {
            let headline = if score.delivered == total {
                "FAHRPLAN PERFEKT ERFÜLLT!".to_string()
            } else {
                "Fahrplan abgeschlossen".to_string()
            };
            (
                headline,
                format!(
                    "{} von {total} Zügen korrekt zugestellt, {} fehlgeleitet.",
                    score.delivered, score.misrouted
                ),
                Color::srgb(0.35, 0.9, 0.45),
            )
        }
    };

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                ..default()
            },
            EndOverlay,
            Name::new("EndOverlay"),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(headline),
                TextFont::from_font_size(48.0),
                TextColor(color),
            ));
            parent.spawn((
                Text::new(detail),
                TextFont::from_font_size(22.0),
                TextColor(Color::srgb(0.85, 0.85, 0.9)),
            ));
            parent.spawn((
                Text::new("R: Neustart"),
                TextFont::from_font_size(18.0),
                TextColor(Color::srgb(0.6, 0.6, 0.65)),
            ));
        });
}

fn spawn_pause_overlay(mut commands: Commands) {
    commands.spawn((
        Text::new("PAUSE"),
        TextFont::from_font_size(36.0),
        TextColor(Color::srgb(0.8, 0.8, 0.85)),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(16.0),
            top: Val::Px(8.0),
            ..default()
        },
        PauseOverlay,
        Name::new("PauseOverlay"),
    ));
}

fn despawn_all<C: Component>(mut commands: Commands, entities: Query<Entity, With<C>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

/// Full reset back to a fresh run — works mid-run and from the end screen.
fn restart_on_r(
    input: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    trains: Query<Entity, With<Train>>,
    mut score: ResMut<Score>,
    mut timetable: ResMut<Timetable>,
    mut graph: ResMut<TrackGraph>,
    mut signals: Query<&mut Signal>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !input.just_pressed(KeyCode::KeyR) {
        return;
    }
    for entity in &trains {
        commands.entity(entity).despawn();
    }
    *score = Score::default();
    *timetable = sim::train::fresh_timetable();
    for switch in &mut graph.switches {
        switch.selected = 0;
    }
    for mut signal in &mut signals {
        signal.green = true;
    }
    commands.remove_resource::<EndCause>();
    next.set(GameState::Playing);
}
