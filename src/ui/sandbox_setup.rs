//! Size picker before creating a new sandbox (M2 §2.2). Its own screen because
//! creating is destructive (it overwrites the single sandbox.ron) — the warning
//! shown here is the confirmation (plan 06 §5). Picking the size resets both
//! places the sandbox lives: the area file AND the autosaved player build.

use bevy::prelude::*;
use stellwerk_sim::layout::Layout;

use super::enter_level;
use super::widgets::{
    BUTTON_BG_PRIMARY, TEXT_BRIGHT, TEXT_DIM, button, despawn_all, text_bundle,
};
use crate::font::UiFont;
use crate::i18n::t;
use crate::levels::{Progress, SANDBOX_ID, empty_sandbox, save_sandbox};
use crate::state::{Editor, GameState};

#[derive(Component)]
struct UiSandboxSetup;

/// A preset carries its dimensions directly — no separate lookup needed.
#[derive(Component, Clone, Copy)]
struct SizePreset {
    w: u32,
    h: u32,
}

/// (label key, w, h). All within `[SANDBOX_MIN, SANDBOX_MAX]` (see `levels`).
const PRESETS: [(&str, u32, u32); 3] = [
    ("sandbox.size_small", 8, 5),
    ("sandbox.size_medium", 12, 7),
    ("sandbox.size_large", 18, 11),
];

pub(super) struct SandboxSetupUiPlugin;

impl Plugin for SandboxSetupUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::SandboxSetup), spawn)
            .add_systems(
                OnExit(GameState::SandboxSetup),
                despawn_all::<UiSandboxSetup>,
            )
            .add_systems(
                Update,
                (pick_size, leave).run_if(in_state(GameState::SandboxSetup)),
            );
    }
}

fn spawn(mut commands: Commands, ui_font: Res<UiFont>) {
    let font = ui_font.0.clone();
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
            UiSandboxSetup,
        ))
        .with_children(|root| {
            root.spawn(text_bundle(&font, t("sandbox.setup_title"), 30.0, TEXT_BRIGHT));
            root.spawn(text_bundle(&font, t("sandbox.setup_warn"), 14.0, TEXT_DIM));
            for (label, w, h) in PRESETS {
                button(root, &font, &t(label), BUTTON_BG_PRIMARY, SizePreset { w, h });
            }
            root.spawn(text_bundle(&font, t("sandbox.back"), 14.0, TEXT_DIM));
        });
}

#[allow(clippy::too_many_arguments)]
fn pick_size(
    presets: Query<(&Interaction, &SizePreset), Changed<Interaction>>,
    mut progress: ResMut<Progress>,
    mut commands: Commands,
    mut editor: ResMut<Editor>,
    mut next: ResMut<NextState<GameState>>,
) {
    for (interaction, preset) in &presets {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // 1) build the fresh area and persist it as sandbox.ron
        let level = empty_sandbox(preset.w, preset.h);
        save_sandbox(&level);
        // 2) reset the OLD player build — otherwise the previous track layout
        //    sticks to the new, smaller area and falls outside it at once.
        progress.entry(SANDBOX_ID).layout = Layout::default();
        progress.save();
        // 3) into the edit phase, like the existing sandbox path
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
}

fn leave(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<GameState>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(GameState::LevelSelect);
    }
}
