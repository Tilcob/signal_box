//! A minimal focusable input for bevy_ui (which ships no text field). Click to
//! focus, type, Enter or click-away commits, Tab commits and jumps to the next
//! field. One component serves two kinds: an integer field (digits only,
//! clamped to `[min, max]`) and a free-text field (a station rename, capped at
//! `max_len`). Reusable beyond the schedule editor.
//!
//! Keys are read from the LOGICAL `ButtonInput<Key>`, never `KeyCode`: on a
//! QWERTZ layout the physical positions differ, and we want the characters the
//! player actually typed (see the editor's undo/redo for the same reasoning).

use bevy::input::keyboard::Key;
use bevy::prelude::*;
use bevy::text::Font;

use super::widgets::{BUTTON_BG, ButtonBase, TEXT_BRIGHT, set_text, text_bundle};
use crate::state::{FocusedField, GameState, not_paused};

/// Cap on typed digits — keeps the parse inside `i64` and the field a sane width.
const MAX_DIGITS: usize = 6;

/// What a [`Field`] edits: a clamped integer or a length-capped string.
#[derive(Clone, Copy)]
enum FieldKind {
    Int { min: i64, max: i64 },
    Text { max_len: usize },
}

/// A focusable input. `editing` is `Some(buffer)` while focused, the buffer
/// being the characters typed so far (pre-filled with the current `text`).
/// `text` is the committed display value — for `Int` it is the canonical
/// decimal of the clamped value. (Name kept for callers that query it directly,
/// e.g. the sandbox size inputs; it now also backs free-text fields.)
#[derive(Component)]
pub(super) struct NumericField {
    kind: FieldKind,
    text: String,
    editing: Option<String>,
}

impl NumericField {
    /// Current committed integer value. Meaningful for `Int` fields (the size /
    /// chapter / order inputs read this); a `Text` field returns 0.
    pub(super) fn value(&self) -> i64 {
        self.text.parse().unwrap_or(0)
    }
}

/// Emitted when an `Int` field commits a (clamped) value — on Enter/Tab or
/// losing focus, and only when the value actually changed.
#[derive(Message)]
pub(super) struct NumericFieldCommit {
    pub field: Entity,
    pub value: i64,
}

/// Emitted when a `Text` field commits a (non-empty, changed) string.
#[derive(Message)]
pub(super) struct TextFieldCommit {
    pub field: Entity,
    pub text: String,
}

pub(super) struct NumericFieldPlugin;

impl Plugin for NumericFieldPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<NumericFieldCommit>()
            .add_message::<TextFieldCommit>()
            .add_systems(
                Update,
                (numeric_field_focus, numeric_field_keys, numeric_field_render)
                    .chain()
                    // Also active on the sandbox setup screen (its size inputs);
                    // there `not_paused` is irrelevant, so only Edit is gated by it.
                    .run_if(
                        in_state(GameState::SandboxSetup)
                            .or(in_state(GameState::Edit).and(not_paused)),
                    ),
            );
    }
}

/// Spawns an integer field carrying the caller's `marker` (e.g. which schedule
/// cell it edits). Looks and feels like a small button showing the number.
pub(super) fn numeric_field<M: Component>(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    value: i64,
    min: i64,
    max: i64,
    marker: M,
) {
    spawn_field(
        parent,
        font,
        FieldKind::Int { min, max },
        value.to_string(),
        marker,
    );
}

/// Spawns a free-text field (e.g. a station rename) capped at `max_len` chars.
pub(super) fn text_field<M: Component>(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    text: &str,
    max_len: usize,
    marker: M,
) {
    spawn_field(parent, font, FieldKind::Text { max_len }, text.to_string(), marker);
}

fn spawn_field<M: Component>(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    kind: FieldKind,
    text: String,
    marker: M,
) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)),
                margin: UiRect::all(Val::Px(2.0)),
                min_width: Val::Px(34.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(BUTTON_BG),
            ButtonBase(BUTTON_BG),
            NumericField {
                kind,
                text: text.clone(),
                editing: None,
            },
            marker,
        ))
        .with_children(|b| {
            b.spawn(text_bundle(font, text, 13.0, TEXT_BRIGHT));
        });
}

/// Whether two field kinds are the same variant (used to scope Tab navigation).
fn same_kind(a: FieldKind, b: FieldKind) -> bool {
    matches!(
        (a, b),
        (FieldKind::Int { .. }, FieldKind::Int { .. })
            | (FieldKind::Text { .. }, FieldKind::Text { .. })
    )
}

/// Parses `buffer` and clamps to `[min, max]`. An empty or unparseable buffer
/// keeps `current` — the field never commits garbage. Pure, so it is unit
/// tested without an app.
fn commit_value(buffer: &str, current: i64, min: i64, max: i64) -> i64 {
    match buffer.parse::<i64>() {
        Ok(n) => n.clamp(min, max),
        Err(_) => current,
    }
}

/// Closes the field's edit session, emitting the matching commit if the value
/// changed. Empty text is treated as "no change" so a name is never blanked.
fn close_field(
    field: &mut NumericField,
    entity: Entity,
    num_commits: &mut MessageWriter<NumericFieldCommit>,
    text_commits: &mut MessageWriter<TextFieldCommit>,
) {
    let Some(buffer) = field.editing.take() else {
        return;
    };
    match field.kind {
        FieldKind::Int { min, max } => {
            let current = field.text.parse::<i64>().unwrap_or(0);
            let value = commit_value(&buffer, current, min, max);
            if value != current {
                field.text = value.to_string();
                num_commits.write(NumericFieldCommit { field: entity, value });
            }
        }
        FieldKind::Text { .. } => {
            let trimmed = buffer.trim();
            if !trimmed.is_empty() && trimmed != field.text {
                field.text = trimmed.to_string();
                text_commits.write(TextFieldCommit {
                    field: entity,
                    text: field.text.clone(),
                });
            }
        }
    }
}

/// Left-click moves focus: commits the old field, opens the clicked one
/// (pre-filling its buffer with the current text). A click on no field at all
/// blurs (and commits) the current one.
pub(super) fn numeric_field_focus(
    mut focus: ResMut<FocusedField>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut fields: Query<(Entity, &Interaction, &mut NumericField)>,
    mut num_commits: MessageWriter<NumericFieldCommit>,
    mut text_commits: MessageWriter<TextFieldCommit>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let target = fields
        .iter()
        .find(|(_, interaction, _)| **interaction == Interaction::Pressed)
        .map(|(entity, _, _)| entity);
    if target == focus.0 {
        return; // re-click on the same (or no) field — nothing to switch
    }
    if let Some(old) = focus.0
        && let Ok((entity, _, mut field)) = fields.get_mut(old)
    {
        close_field(&mut field, entity, &mut num_commits, &mut text_commits);
    }
    if let Some(new) = target
        && let Ok((_, _, mut field)) = fields.get_mut(new)
    {
        field.editing = Some(field.text.clone());
    }
    focus.0 = target;
}

/// Typing edits the focused field's buffer; Enter commits and blurs; Tab
/// commits and advances to the next field. A focus pointing at a despawned
/// field (panel rebuilt after a commit) is cleared so the hotkeys un-gate.
fn numeric_field_keys(
    mut focus: ResMut<FocusedField>,
    keys: Res<ButtonInput<Key>>,
    mut fields: Query<(Entity, &mut NumericField)>,
    mut num_commits: MessageWriter<NumericFieldCommit>,
    mut text_commits: MessageWriter<TextFieldCommit>,
) {
    let Some(focused) = focus.0 else {
        return;
    };
    let Ok((_, mut field)) = fields.get_mut(focused) else {
        focus.0 = None;
        return;
    };
    let kind = field.kind;
    let Some(buffer) = field.editing.as_mut() else {
        return;
    };
    let (cap, accept): (usize, fn(char) -> bool) = match kind {
        FieldKind::Int { .. } => (MAX_DIGITS, |c| c.is_ascii_digit()),
        FieldKind::Text { max_len } => (max_len, |c| !c.is_control()),
    };
    let mut commit = false;
    let mut tab = false;
    for key in keys.get_just_pressed() {
        match key {
            Key::Character(s) => {
                for ch in s.chars().filter(|c| accept(*c)) {
                    if buffer.chars().count() < cap {
                        buffer.push(ch);
                    }
                }
            }
            // Space types into a text field and deliberately does NOT commit or
            // blur — load-bearing: the edit HUD uses Space to start the run,
            // gated on `no_field_focused`, so a focused field must keep focus on
            // Space or the run would start mid-edit (see `edit_hud::start_button`).
            Key::Space
                if matches!(kind, FieldKind::Text { .. }) && buffer.chars().count() < cap =>
            {
                buffer.push(' ');
            }
            Key::Backspace => {
                buffer.pop();
            }
            Key::Enter => commit = true,
            Key::Tab => tab = true,
            _ => {}
        }
    }
    if !commit && !tab {
        return;
    }
    if let Ok((entity, mut field)) = fields.get_mut(focused) {
        close_field(&mut field, entity, &mut num_commits, &mut text_commits);
    }
    focus.0 = None;
    if tab {
        // "Next" = next field of the SAME kind in entity order. On a fresh panel
        // entity order is spawn order (rows top-to-bottom, fields left-to-right).
        // Restricting to the same kind keeps Tab inside one panel: on the Edit
        // screen the schedule holds the only Int fields and the station panel the
        // only Text fields, so same-kind == same-panel without a panel id.
        // ponytail: entity order + kind, not an explicit tab index — could
        // scramble if a panel ever mixes kinds or after heavy respawn churn; add
        // an index field then.
        let mut entities: Vec<Entity> = fields
            .iter()
            .filter(|(_, f)| same_kind(f.kind, kind))
            .map(|(e, _)| e)
            .collect();
        entities.sort();
        let next = entities
            .iter()
            .copied()
            .find(|&e| e > focused)
            .or_else(|| entities.first().copied());
        if let Some(next) = next
            && let Ok((_, mut field)) = fields.get_mut(next)
        {
            field.editing = Some(field.text.clone());
            focus.0 = Some(next);
        }
    }
}

/// Mirrors each changed field into its text child: the live buffer (with a
/// caret) while editing, otherwise the committed text.
fn numeric_field_render(
    fields: Query<(&NumericField, &Children), Changed<NumericField>>,
    mut texts: Query<&mut Text>,
) {
    for (field, children) in &fields {
        let label = match &field.editing {
            Some(buffer) => format!("{buffer}_"),
            None => field.text.clone(),
        };
        if let Some(&child) = children.first()
            && let Ok(mut text) = texts.get_mut(child)
        {
            set_text(&mut text, label);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::commit_value;

    #[test]
    fn clamps_into_range() {
        assert_eq!(commit_value("999", 100, 1, 499), 499);
        assert_eq!(commit_value("0", 100, 1, 499), 1);
        assert_eq!(commit_value("250", 100, 1, 499), 250);
    }

    #[test]
    fn empty_or_garbage_keeps_current() {
        assert_eq!(commit_value("", 80, 0, 99999), 80);
        assert_eq!(commit_value("--", 80, 0, 99999), 80);
    }
}
