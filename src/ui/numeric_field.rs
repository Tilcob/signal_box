//! A minimal focusable numeric input for bevy_ui (which ships no text field).
//! Click to focus, type digits, Enter or click-away commits the value clamped
//! to `[min, max]`. Reusable beyond the schedule editor.
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

/// A focusable integer input. `editing` is `Some(buffer)` while focused, the
/// buffer being the digits typed so far (pre-filled with the current value).
#[derive(Component)]
pub(super) struct NumericField {
    pub value: i64,
    min: i64,
    max: i64,
    editing: Option<String>,
}

/// Emitted when a field commits a (clamped) value — on Enter or losing focus,
/// and only when the value actually changed.
#[derive(Message)]
pub(super) struct NumericFieldCommit {
    pub field: Entity,
    pub value: i64,
}

pub(super) struct NumericFieldPlugin;

impl Plugin for NumericFieldPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<NumericFieldCommit>().add_systems(
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

/// Spawns a numeric field carrying the caller's `marker` (e.g. which schedule
/// cell it edits). Looks and feels like a small button showing the number.
pub(super) fn numeric_field<M: Component>(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    value: i64,
    min: i64,
    max: i64,
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
                value,
                min,
                max,
                editing: None,
            },
            marker,
        ))
        .with_children(|b| {
            b.spawn(text_bundle(font, value.to_string(), 13.0, TEXT_BRIGHT));
        });
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

/// Closes the field's edit session, emitting a commit if the value changed.
fn close_field(
    field: &mut NumericField,
    entity: Entity,
    commits: &mut MessageWriter<NumericFieldCommit>,
) {
    let Some(buffer) = field.editing.take() else {
        return;
    };
    let value = commit_value(&buffer, field.value, field.min, field.max);
    if value != field.value {
        field.value = value;
        commits.write(NumericFieldCommit { field: entity, value });
    }
}

/// Left-click moves focus: commits the old field, opens the clicked one
/// (pre-filling its buffer with the current value). A click on no field at all
/// blurs (and commits) the current one.
pub(super) fn numeric_field_focus(
    mut focus: ResMut<FocusedField>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut fields: Query<(Entity, &Interaction, &mut NumericField)>,
    mut commits: MessageWriter<NumericFieldCommit>,
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
        close_field(&mut field, entity, &mut commits);
    }
    if let Some(new) = target
        && let Ok((_, _, mut field)) = fields.get_mut(new)
    {
        field.editing = Some(field.value.to_string());
    }
    focus.0 = target;
}

/// Digit/Backspace edit the focused field's buffer; Enter commits and blurs.
/// A focus pointing at a despawned field (panel rebuilt after a commit) is
/// cleared so the hotkeys un-gate.
fn numeric_field_keys(
    mut focus: ResMut<FocusedField>,
    keys: Res<ButtonInput<Key>>,
    mut fields: Query<(Entity, &mut NumericField)>,
    mut commits: MessageWriter<NumericFieldCommit>,
) {
    let Some(focused) = focus.0 else {
        return;
    };
    let Ok((entity, mut field)) = fields.get_mut(focused) else {
        focus.0 = None;
        return;
    };
    let Some(buffer) = field.editing.as_mut() else {
        return;
    };
    let mut commit = false;
    for key in keys.get_just_pressed() {
        match key {
            Key::Character(s) => {
                for ch in s.chars().filter(|c| c.is_ascii_digit()) {
                    if buffer.len() < MAX_DIGITS {
                        buffer.push(ch);
                    }
                }
            }
            Key::Backspace => {
                buffer.pop();
            }
            Key::Enter => commit = true,
            _ => {}
        }
    }
    if commit {
        close_field(&mut field, entity, &mut commits);
        focus.0 = None;
    }
}

/// Mirrors each changed field into its text child: the live buffer (with a
/// caret) while editing, otherwise the committed value.
fn numeric_field_render(
    fields: Query<(&NumericField, &Children), Changed<NumericField>>,
    mut texts: Query<&mut Text>,
) {
    for (field, children) in &fields {
        let label = match &field.editing {
            Some(buffer) => format!("{buffer}_"),
            None => field.value.to_string(),
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
