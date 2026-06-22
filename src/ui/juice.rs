//! Small juice effects, hand-rolled (GDD §12.3 — no `bevy_tweening`):
//! a mode-switch fade and a staggered medal "pop". The route-formation glow
//! pulse lives in `board::run_board` because it rides the per-frame band
//! recolour there.

use bevy::prelude::*;

use crate::state::GameState;

const FADE_SECS: f32 = 0.22;
const POP_SECS: f32 = 0.28;

/// Full-screen overlay that fades from the desk colour to transparent on every
/// Edit/Run entry — softens the hard state cut.
#[derive(Component)]
struct ModeFade(Timer);

/// Grows a UI node from 0 to its target size with a slight overshoot, after an
/// optional delay (for staggering). Animates `Node` size, not `Transform`
/// scale, because the UI layout owns the transform of UI nodes.
#[derive(Component)]
pub(crate) struct Pop {
    elapsed: f32,
    delay: f32,
    size: f32,
}

impl Pop {
    pub(crate) fn new(delay: f32, size: f32) -> Self {
        Self {
            elapsed: 0.0,
            delay,
            size,
        }
    }
}

/// Ease-out-back: 0→1 with a small overshoot past 1, the classic "pop".
fn ease_out_back(t: f32) -> f32 {
    const C1: f32 = 1.70158;
    const C3: f32 = C1 + 1.0;
    let p = t - 1.0;
    1.0 + C3 * p * p * p + C1 * p * p
}

fn spawn_mode_fade(mut commands: Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        // Matches the window ClearColor so the fade reads as the desk itself.
        BackgroundColor(Color::srgb(0.013, 0.016, 0.022)),
        GlobalZIndex(1000),
        ModeFade(Timer::from_seconds(FADE_SECS, TimerMode::Once)),
    ));
}

fn drive_mode_fade(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut ModeFade, &mut BackgroundColor)>,
) {
    for (e, mut fade, mut bg) in &mut q {
        fade.0.tick(time.delta());
        bg.0.set_alpha(1.0 - fade.0.fraction());
        if fade.0.is_finished() {
            commands.entity(e).despawn();
        }
    }
}

fn drive_pop(time: Res<Time>, mut q: Query<(&mut Pop, &mut Node)>) {
    for (mut pop, mut node) in &mut q {
        pop.elapsed += time.delta_secs();
        let t = ((pop.elapsed - pop.delay) / POP_SECS).clamp(0.0, 1.0);
        let s = ease_out_back(t) * pop.size;
        node.width = Val::Px(s);
        node.height = Val::Px(s);
    }
}

pub(super) struct JuicePlugin;

impl Plugin for JuicePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Edit), spawn_mode_fade)
            .add_systems(OnEnter(GameState::Run), spawn_mode_fade)
            .add_systems(Update, (drive_mode_fade, drive_pop));
    }
}
