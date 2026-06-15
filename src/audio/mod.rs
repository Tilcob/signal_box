//! Audio: `bevy_kira_audio` for fades, loops and many concurrent
//! one-shots — the control the ASMR "Pult" sound design needs (kira was chosen
//! over Bevy's built-in audio for exactly this). Two channels give music
//! and SFX independent volume. Bevy's own audio plugin is disabled in `main`
//! so the two backends don't fight over the output device.

mod assets;
mod music;
mod sfx;

pub use sfx::SfxKind;

use bevy::prelude::*;
use bevy_kira_audio::{AudioApp, AudioPlugin};

use crate::state::GameState;

/// Music channel marker (looping background tracks, one at a time).
#[derive(Resource)]
pub struct MusicChannel;
/// SFX channel marker (short one-shots, may overlap).
#[derive(Resource)]
pub struct SfxChannel;

pub struct AudioManagerPlugin;

impl Plugin for AudioManagerPlugin {
    fn build(&self, app: &mut App) {
        // Backend + channels first: this registers the ogg/wav `AudioSource`
        // loader, so the `asset_server.load(...)` calls below find a loader.
        app.add_plugins(AudioPlugin)
            .add_audio_channel::<MusicChannel>()
            .add_audio_channel::<SfxChannel>();

        // Insert AudioAssets at BUILD time — it must exist before the initial
        // OnEnter(MainMenu), which bevy_state fires before PreStartup. A Startup
        // system is too late and leaves the menu music silent (see
        // `assets::build_audio_assets` + `src/font.rs` for the same race).
        let assets = assets::build_audio_assets(app.world().resource::<AssetServer>());
        app.insert_resource(assets);

        app.init_resource::<music::CurrentTrack>()
            .init_resource::<music::LevelPlaylist>()
            .add_observer(sfx::on_sfx)
            .add_systems(Update, sfx::button_click_sfx)
            // Menu-side states share the calm menu loop; the desk (Edit/Run) runs
            // a random non-repeating playlist that persists across Edit<->Run.
            .add_systems(OnEnter(GameState::MainMenu), music::menu_music)
            .add_systems(OnEnter(GameState::LevelSelect), music::menu_music)
            .add_systems(OnEnter(GameState::Edit), music::start_level_playlist)
            .add_systems(OnEnter(GameState::Run), music::start_level_playlist)
            .add_systems(
                Update,
                // Result keeps the desk playlist alive too, so the music does not
                // cut out (or fall silent mid-gap) on the outcome screen.
                music::drive_level_playlist.run_if(
                    in_state(GameState::Edit)
                        .or(in_state(GameState::Run))
                        .or(in_state(GameState::Result)),
                ),
            );
    }
}
