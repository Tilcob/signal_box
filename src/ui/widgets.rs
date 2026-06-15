//! Shared UI theme and widget helpers: colors, text/button constructors and
//! the global hover/press feedback.

use bevy::prelude::*;
use bevy::text::Font;

pub(super) const PANEL_BG: Color = Color::srgba(0.05, 0.06, 0.08, 0.92);
pub(super) const BUTTON_BG: Color = Color::srgb(0.10, 0.12, 0.16);
pub(super) const BUTTON_BG_PRIMARY: Color = Color::srgb(0.10, 0.22, 0.14);
pub(super) const BUTTON_BG_BLOCKED: Color = Color::srgb(0.22, 0.10, 0.10);
pub(super) const TEXT_DIM: Color = Color::srgb(0.55, 0.58, 0.65);
pub(super) const TEXT_BRIGHT: Color = Color::srgb(0.88, 0.90, 0.95);
/// Achieved-par medal fill.
pub(super) const MEDAL: Color = Color::srgb(0.95, 0.80, 0.38);
/// Solved-level marker (matches the success headline green).
pub(super) const SOLVED: Color = Color::srgb(0.40, 1.0, 0.55);

/// A status dot drawn as a UI shape, not a font glyph — the DIN UI font has no
/// ●/○ (restfeature 04). `filled` = solid disc in `color`, else a dim hollow
/// ring. Used for par medals and the solved marker; PNG icons replace these
/// before release.
pub(super) fn dot(parent: &mut ChildSpawnerCommands, filled: bool, color: Color) {
    let (bg, border) = if filled {
        (color, color)
    } else {
        (Color::NONE, TEXT_DIM)
    };
    parent.spawn((
        Node {
            width: Val::Px(10.0),
            height: Val::Px(10.0),
            margin: UiRect::all(Val::Px(2.0)),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(bg),
        BorderColor::all(border),
    ));
}

/// Like [`button`] but the caller fills the inner row (text plus icon nodes
/// like [`dot`]) instead of a single label.
pub(super) fn button_row<M: Component>(
    parent: &mut ChildSpawnerCommands,
    bg: Color,
    marker: M,
    children: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(7.0)),
                margin: UiRect::all(Val::Px(3.0)),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(bg),
            ButtonBase(bg),
            marker,
        ))
        .with_children(children);
}

/// Status line shared by the level select and the result overlay.
#[derive(Component)]
pub(super) struct StatusText;

/// Resting background color of a button — [`button_feedback`] lightens it on
/// hover/press and restores it on release.
#[derive(Component)]
pub(super) struct ButtonBase(pub(super) Color);

/// Buttons whose visual state needs a refresh this frame.
type ChangedButton = (With<Button>, Or<(Changed<Interaction>, Changed<ButtonBase>)>);

pub(super) fn button_feedback(
    mut buttons: Query<(&Interaction, &ButtonBase, &mut BackgroundColor), ChangedButton>,
) {
    for (interaction, base, mut bg) in &mut buttons {
        let color = match interaction {
            Interaction::Pressed => lift(base.0, 0.25),
            Interaction::Hovered => lift(base.0, 0.10),
            Interaction::None => base.0,
        };
        *bg = BackgroundColor(color);
    }
}

/// Mixes a color toward white — visible feedback on the near-black theme.
fn lift(color: Color, amount: f32) -> Color {
    let c = color.to_srgba();
    Color::srgba(
        c.red + (1.0 - c.red) * amount,
        c.green + (1.0 - c.green) * amount,
        c.blue + (1.0 - c.blue) * amount,
        c.alpha,
    )
}

pub(super) fn despawn_all<C: Component>(mut commands: Commands, q: Query<Entity, With<C>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

/// Writes `value` only when it differs — HUD texts are recomputed every
/// frame, and unconditional assignment forces a re-shape (and glyph atlas
/// churn) per frame even though nothing changed.
pub(super) fn set_text(text: &mut Text, value: String) {
    if text.0 != value {
        text.0 = value;
    }
}

/// All text goes through here with the explicit [`crate::font::UiFont`]
/// handle — see the `UiFont` docs for why the default-font handle must not
/// be touched.
pub(super) fn text_bundle(font: &Handle<Font>, value: String, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}

pub(super) fn button<M: Component>(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    label: &str,
    bg: Color,
    marker: M,
) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(7.0)),
                margin: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(bg),
            ButtonBase(bg),
            marker,
        ))
        .with_children(|b| {
            b.spawn(text_bundle(font, label.to_string(), 16.0, TEXT_BRIGHT));
        });
}

pub(super) fn small_button<M: Component>(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    label: &str,
    marker: M,
) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)),
                margin: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(BUTTON_BG),
            ButtonBase(BUTTON_BG),
            marker,
        ))
        .with_children(|b| {
            b.spawn(text_bundle(font, label.to_string(), 13.0, TEXT_BRIGHT));
        });
}
