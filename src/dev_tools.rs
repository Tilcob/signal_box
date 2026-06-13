//! Dev-only tooling (feature `dev`): F12 world inspector, F3 FPS overlay,
//! and the STELLWERK_AUTOCYCLE soak test.

use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig};
use bevy::input::common_conditions::input_toggle_active;
use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;

use crate::levels::{Catalog, Progress};
use crate::run::RunCtl;
use crate::state::{ActiveLevel, Editor, GameState, Tool};

pub struct DevToolsPlugin;

impl Plugin for DevToolsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .add_plugins(
                WorldInspectorPlugin::new().run_if(input_toggle_active(false, KeyCode::F12)),
            )
            .add_plugins(FpsOverlayPlugin {
                config: FpsOverlayConfig {
                    enabled: false,
                    // The graph has its own flag and DEFAULTS TO ON — without
                    // this it renders its red/orange bars in the top-left
                    // corner even though the overlay is "disabled".
                    frame_time_graph_config: FrameTimeGraphConfig {
                        enabled: false,
                        ..default()
                    },
                    ..default()
                },
            })
            .add_systems(Update, toggle_fps);
        // Soak test (STELLWERK_AUTOCYCLE=1): automatically cycle through the
        // catalog — LevelSelect → Edit → (Run → Result) → back — forever.
        // Reproduces the "viel Menü wechseln" font-corruption report so the
        // fix has a regression harness. Pairs well with STELLWERK_WINDOWED.
        if std::env::var_os("STELLWERK_AUTOCYCLE").is_some() {
            app.add_systems(Update, auto_cycle);
        }
    }
}

fn toggle_fps(input: Res<ButtonInput<KeyCode>>, mut config: ResMut<FpsOverlayConfig>) {
    if input.just_pressed(KeyCode::F3) {
        config.enabled = !config.enabled;
        config.frame_time_graph_config.enabled = config.enabled;
    }
}

/// Mirrors the user flow: LevelSelect → enter level N → back to LevelSelect,
/// advancing N each round. Same setup as `ui::enter_level`.
#[allow(clippy::too_many_arguments)]
fn auto_cycle(
    time: Res<Time>,
    state: Res<State<GameState>>,
    catalog: Option<Res<Catalog>>,
    progress: Res<Progress>,
    mut editor: ResMut<Editor>,
    ctl: Option<ResMut<RunCtl>>,
    mut commands: Commands,
    mut next: ResMut<NextState<GameState>>,
    mut timer: Local<f32>,
    mut round: Local<usize>,
) {
    // Fast-forward running sims so outcomes (and Result screens) happen
    // within the soak cadence.
    if let Some(mut ctl) = ctl
        && ctl.speed == 1
    {
        ctl.speed = 16;
    }
    *timer += time.delta_secs();
    if *timer < 0.5 {
        return;
    }
    match state.get() {
        // Drive the boot flow once: MainMenu → Loading → (auto) LevelSelect.
        GameState::MainMenu => {
            *timer = 0.0;
            next.set(GameState::Loading);
        }
        // Loading hands off to LevelSelect on its own (see crate::loading).
        GameState::Loading => {}
        // Not part of the autocycle boot flow.
        GameState::SandboxSetup => {}
        GameState::LevelSelect => {
            *timer = 0.0;
            let Some(catalog) = catalog.as_ref().filter(|c| !c.0.is_empty()) else {
                return;
            };
            let entry = &catalog.0[*round % catalog.0.len()];
            *round += 1;
            editor.layout = progress
                .levels
                .get(&entry.id)
                .map(|p| p.layout.clone())
                .unwrap_or_default();
            editor.undo.clear();
            editor.redo.clear();
            editor.tool = Tool::Track;
            editor.variant = 0;
            editor.selected_switch = None;
            editor.drag = None;
            commands.insert_resource(ActiveLevel {
                id: entry.id.clone(),
                index: *round - 1,
                level: entry.level.clone(),
                briefing: entry.meta.briefing.clone(),
                sandbox: false,
            });
            next.set(GameState::Edit);
            info!("autocycle: round {} → {}", *round, entry.id);
        }
        GameState::Edit => {
            *timer = 0.0;
            // Alternate: every other round actually runs the level —
            // covering Run + Result + the Esc cleanup paths.
            if *round % 2 == 1 {
                next.set(GameState::Run);
            } else {
                next.set(GameState::LevelSelect);
            }
        }
        GameState::Run => {
            // Let the run play ~2s (tick HUD churns text), then Esc out.
            if *timer >= 2.0 {
                *timer = 0.0;
                next.set(GameState::Edit);
            }
        }
        GameState::Result => {
            *timer = 0.0;
            next.set(GameState::LevelSelect);
        }
    }
}
