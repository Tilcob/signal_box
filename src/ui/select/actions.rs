//! Global level-select actions: open/replace the sandbox, import a code, and
//! the language toggle — plus the localized decode-error text for imports.

use bevy::prelude::*;
use stellwerk_codes::{DecodeError, Payload};

use crate::clipboard::PasteError;
use crate::i18n::{set_lang, t};
use crate::levels::{Catalog, Progress, SANDBOX_ID, load_sandbox, save_sandbox};
use crate::state::{Editor, GameState};
use crate::ui::enter_level;

use super::{ImportButton, LangButton, MainMenuButton, NewSandboxButton, SandboxButton, UiStatus};

#[allow(clippy::too_many_arguments)]
pub(super) fn select_buttons(
    sandbox: Query<&Interaction, (Changed<Interaction>, With<SandboxButton>)>,
    new_sandbox: Query<&Interaction, (Changed<Interaction>, With<NewSandboxButton>)>,
    import: Query<&Interaction, (Changed<Interaction>, With<ImportButton>)>,
    lang: Query<&Interaction, (Changed<Interaction>, With<LangButton>)>,
    main_menu: Query<&Interaction, (Changed<Interaction>, With<MainMenuButton>)>,
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
            String::new(),
            true,
            &progress,
            &mut commands,
            &mut editor,
            &mut next,
        );
        return;
    }
    if new_sandbox.iter().any(|i| *i == Interaction::Pressed) {
        next.set(GameState::SandboxSetup);
        return;
    }
    if main_menu.iter().any(|i| *i == Interaction::Pressed) {
        next.set(GameState::MainMenu);
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
        match crate::clipboard::paste() {
            Err(PasteError::Empty) => status.0 = t("select.import_clipboard_empty"),
            Err(PasteError::Unavailable(e)) => {
                status.0 = format!("{}: {e}", t("import.error.clipboard"))
            }
            Ok(text) => match stellwerk_codes::decode(&text) {
                Err(e) => status.0 = decode_error_text(&e),
                Ok(Payload::Solution { level_id, layout }) => {
                    if level_id == SANDBOX_ID || catalog.0.iter().any(|entry| entry.id == level_id)
                    {
                        progress.entry(&level_id).layout = layout;
                        progress.save();
                        status.0 = format!("{}{level_id}", t("select.import_ok"));
                    } else {
                        status.0 = format!("{}{level_id}", t("select.import_unknown"));
                    }
                }
                Ok(Payload::Level { level }) => {
                    save_sandbox(&level);
                    status.0 = format!("{}{}", t("select.import_sandbox"), level.name);
                }
            },
        }
    }
}

/// Every key [`decode_error_text`] can emit — kept beside the match so the
/// i18n coverage checker (see `crate::i18n` tests) asserts all of them resolve
/// in both languages. Adding a [`DecodeError`] variant breaks the exhaustive
/// match below and reminds you to extend this.
#[cfg(test)]
pub(crate) const DECODE_ERROR_KEYS: &[&str] = &[
    "import.error.prefix",
    "import.error.base64",
    "import.error.version",
    "import.error.corrupt",
];

/// Static chapter-navigation keys (the chapter names themselves use `t_or`
/// with an authored fallback, like level names, so they are not required here).
#[cfg(test)]
pub(crate) const SELECT_CHAPTER_KEYS: &[&str] =
    &["select.chapter_hint", "select.chapter_back", "select.main_menu", "select.sandbox_hint"];

/// Localized import-failure text. `DecodeError`'s own `Display` stays English
/// (logs); the player-facing message is translated here — same split as
/// `valerr::valerr_text` for `ValidationError`.
fn decode_error_text(e: &DecodeError) -> String {
    match e {
        DecodeError::Prefix => t("import.error.prefix"),
        DecodeError::Base64 => t("import.error.base64"),
        DecodeError::Version(v) => format!("{} ({v})", t("import.error.version")),
        DecodeError::Corrupt => t("import.error.corrupt"),
    }
}
