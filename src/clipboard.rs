//! System clipboard for sharing codes (M2 restfeature 01), with a file
//! fallback so headless/CI runs and Linux sessions without a clipboard manager
//! still work.
//!
//! `stellwerk_codes` produces the strings; this module only moves them in and
//! out of the OS clipboard. Nothing here panics — a failed clipboard degrades
//! to the `stellwerk_code.txt` / `stellwerk_import.txt` files the game used
//! before, and the caller turns the outcome into a localized status line.

use std::path::PathBuf;

/// Export target the game writes the code to.
const EXPORT_FILE: &str = "stellwerk_code.txt";
/// Import source the game reads a code from when the clipboard is empty.
const IMPORT_FILE: &str = "stellwerk_import.txt";

/// Where a [`copy`] ended up — the caller maps this to an i18n status key.
pub enum CopyOutcome {
    /// Code is in the system clipboard.
    Clipboard,
    /// Clipboard unavailable; code written to this file instead.
    File(PathBuf),
    /// Neither path worked; the string carries the errors for the status line.
    Failed(String),
}

/// Why a [`paste`] produced no usable text.
pub enum PasteError {
    /// Clipboard blank and the import file empty/absent — nothing to import.
    Empty,
    /// Both the clipboard and the file errored; string is the file error.
    Unavailable(String),
}

/// Puts `text` on the system clipboard, falling back to [`EXPORT_FILE`].
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
