//! System clipboard for sharing codes.
//!
//! Desktop: the OS clipboard via `arboard`, with a file fallback so headless/CI
//! runs and Linux sessions without a clipboard manager still work. Browser
//! (wasm): the Web Clipboard API for copy and `window.prompt` for paste (a
//! synchronous read that sidesteps the async clipboard-read permission dance);
//! no file fallback there.
//!
//! `stellwerk_codes` produces the strings; this module only moves them in and
//! out. Nothing here panics — the caller turns the outcome into a localized
//! status line. The split is target-gated so the desktop build is unchanged.

use std::path::PathBuf;

/// Where a [`copy`] ended up — the caller maps this to an i18n status key.
pub enum CopyOutcome {
    /// Code is in the system clipboard.
    Clipboard,
    /// Clipboard unavailable; code written to this file instead (desktop only;
    /// never constructed on wasm, which has no file fallback).
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    File(PathBuf),
    /// Neither path worked; the string carries the errors for the status line.
    Failed(String),
}

/// Why a [`paste`] produced no usable text.
pub enum PasteError {
    /// Clipboard blank and the import file empty/absent — nothing to import.
    Empty,
    /// Both the clipboard and the file errored; string is the error.
    Unavailable(String),
}

// --- Desktop ----------------------------------------------------------------

/// Export target the game writes the code to when the clipboard fails.
#[cfg(not(target_arch = "wasm32"))]
const EXPORT_FILE: &str = "stellwerk_code.txt";
/// Import source the game reads a code from when the clipboard is empty.
#[cfg(not(target_arch = "wasm32"))]
const IMPORT_FILE: &str = "stellwerk_import.txt";

/// Puts `text` on the system clipboard, falling back to [`EXPORT_FILE`].
#[cfg(not(target_arch = "wasm32"))]
pub fn copy(text: &str) -> CopyOutcome {
    match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text.to_owned())) {
        Ok(()) => CopyOutcome::Clipboard,
        Err(clip_err) => match std::fs::write(EXPORT_FILE, text) {
            Ok(()) => CopyOutcome::File(PathBuf::from(EXPORT_FILE)),
            Err(file_err) => CopyOutcome::Failed(format!("{clip_err}; {file_err}")),
        },
    }
}

/// Reads a code from the system clipboard, falling back to [`IMPORT_FILE`].
///
/// A blank/whitespace clipboard is treated as "nothing there" so a present
/// import file still wins — the file is the deliberate manual override.
#[cfg(not(target_arch = "wasm32"))]
pub fn paste() -> Result<String, PasteError> {
    if let Ok(text) = arboard::Clipboard::new().and_then(|mut cb| cb.get_text())
        && !text.trim().is_empty()
    {
        return Ok(text);
    }
    match std::fs::read_to_string(IMPORT_FILE) {
        Ok(text) if !text.trim().is_empty() => Ok(text),
        Ok(_) => Err(PasteError::Empty),
        Err(e) => Err(PasteError::Unavailable(e.to_string())),
    }
}

// --- Browser (wasm) ---------------------------------------------------------

/// Copies `text` via a hidden textarea + `document.execCommand("copy")`. This
/// is synchronous, works inside the itch iframe (unlike the async,
/// permission-gated Clipboard API), and — crucially — reports whether the copy
/// actually happened, so we never claim success when the browser blocked it.
#[cfg(target_arch = "wasm32")]
pub fn copy(text: &str) -> CopyOutcome {
    use wasm_bindgen::JsCast;
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return CopyOutcome::Failed("no document".into());
    };
    let Some(body) = document.body() else {
        return CopyOutcome::Failed("no body".into());
    };
    let Some(textarea) = document
        .create_element("textarea")
        .ok()
        .and_then(|el| el.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
    else {
        return CopyOutcome::Failed("no textarea".into());
    };
    textarea.set_value(text);
    let _ = body.append_child(&textarea);
    textarea.select();
    // `exec_command` lives on `HtmlDocument`, not the base `Document`.
    let ok = document
        .dyn_into::<web_sys::HtmlDocument>()
        .ok()
        .and_then(|doc| doc.exec_command("copy").ok())
        .unwrap_or(false);
    let _ = body.remove_child(&textarea);
    if ok {
        CopyOutcome::Clipboard
    } else {
        CopyOutcome::Failed("clipboard blocked".into())
    }
}

/// Reads a code via `window.prompt` — a synchronous dialog, so it avoids the
/// async, permission-gated clipboard *read* API (and works inside the itch
/// iframe). The player pastes into the prompt with Ctrl+V.
#[cfg(target_arch = "wasm32")]
pub fn paste() -> Result<String, PasteError> {
    let Some(window) = web_sys::window() else {
        return Err(PasteError::Unavailable("no window".into()));
    };
    match window.prompt_with_message("Code:") {
        Ok(Some(text)) if !text.trim().is_empty() => Ok(text),
        Ok(_) => Err(PasteError::Empty),
        Err(_) => Err(PasteError::Unavailable("prompt blocked".into())),
    }
}
