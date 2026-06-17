//! Size picker before creating a new sandbox. Its own screen because
//! creating is destructive (it overwrites the single sandbox.ron) — the warning
//! shown here is the confirmation. Width and height are typed into numeric
//! fields (clamped to `[SANDBOX_MIN, SANDBOX_MAX]`); the on-screen range hint
//! tells the player the bounds, so the clamp can never surprise them. Creating
//! resets both places the sandbox lives: the area file AND the autosaved build.

use bevy::prelude::*;
use stellwerk_sim::layout::Layout;

use super::enter_level;
use super::numeric_field::{NumericField, numeric_field, numeric_field_focus};
use super::widgets::{
    BUTTON_BG_PRIMARY, TEXT_BRIGHT, TEXT_DIM, button, despawn_all, text_bundle,
};
use crate::font::UiFont;
use crate::i18n::t;
use crate::levels::{
    Progress, SANDBOX_ID, SANDBOX_MAX, SANDBOX_MIN, empty_sandbox, save_sandbox,
};
use crate::state::{Editor, FocusedField, GameState};

#[derive(Component)]
struct UiSandboxSetup;

#[derive(Component)]
struct WidthField;

#[derive(Component)]
struct HeightField;

#[derive(Component)]
struct CreateSandbox;

/// Defaults match the old "Medium" preset.
const DEFAULT_W: i64 = 12;
const DEFAULT_H: i64 = 7;

pub(super) struct SandboxSetupUiPlugin;

impl Plugin for SandboxSetupUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::SandboxSetup), spawn)
            .add_systems(
                OnExit(GameState::SandboxSetup),
                (despawn_all::<UiSandboxSetup>, clear_focus),
            )
            .add_systems(
                Update,
                // `create` after `numeric_field_focus`: clicking Create blurs the
                // focused field, which commits its typed buffer into `.value`.
                // Reading before that commit would use the stale value.
                (create.after(numeric_field_focus), leave)
                    .run_if(in_state(GameState::SandboxSetup)),
            );
    }
}

fn spawn(mut commands: Commands, ui_font: Res<UiFont>) {
    let font = ui_font.0.clone();
    let range = format!("{} ({}-{})", t("sandbox.size_range"), SANDBOX_MIN, SANDBOX_MAX);
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
            root.spawn(text_bundle(&font, range, 14.0, TEXT_DIM));
            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|row| {
                let min = SANDBOX_MIN as i64;
                let max = SANDBOX_MAX as i64;
                row.spawn(text_bundle(&font, t("sandbox.size_width"), 14.0, TEXT_DIM));
                numeric_field(row, &font, DEFAULT_W, min, max, WidthField);
                row.spawn(text_bundle(&font, t("sandbox.size_height"), 14.0, TEXT_DIM));
                numeric_field(row, &font, DEFAULT_H, min, max, HeightField);
            });
            button(root, &font, &t("sandbox.create"), BUTTON_BG_PRIMARY, CreateSandbox);
            root.spawn(text_bundle(&font, t("sandbox.back"), 14.0, TEXT_DIM));
        });
}

#[allow(clippy::too_many_arguments)]
fn create(
    create_btn: Query<&Interaction, (With<CreateSandbox>, Changed<Interaction>)>,
    width: Query<&NumericField, With<WidthField>>,
    height: Query<&NumericField, With<HeightField>>,
    mut progress: ResMut<Progress>,
    mut commands: Commands,
    mut editor: ResMut<Editor>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !create_btn.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let (Ok(w), Ok(h)) = (width.single(), height.single()) else {
        return;
    };
    // The fields are clamped to `[SANDBOX_MIN, SANDBOX_MAX]`, so `value` is a
    // small positive integer; `empty_sandbox` clamps again as a backstop.
    let level = empty_sandbox(w.value() as u32, h.value() as u32);
    save_sandbox(&level);
    // Reset the OLD player build — otherwise the previous track layout sticks to
    // the new, smaller area and falls outside it at once.
    progress.entry(SANDBOX_ID).layout = Layout::default();
    progress.save();
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
}

fn leave(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<GameState>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(GameState::LevelSelect);
    }
}

/// Drop focus on the way out so a despawned field can't linger in
/// `FocusedField` and gate hotkeys in the next screen.
fn clear_focus(mut focus: ResMut<FocusedField>) {
    focus.0 = None;
}
