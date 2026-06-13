//! The static edit-mode board: buildable grid with cell indices, stations,
//! fixed and player layout.

use bevy::prelude::*;

use super::draw::{BoardGfx, Tag, draw_layout, draw_stations};
use super::geometry::{CELL, cell_world};
use super::palette::{col_cell_index, col_fixed, col_grid, col_player};
use crate::font::UiFont;
use crate::state::{ActiveLevel, Editor};

pub(super) fn rebuild_edit_board(
    mut commands: Commands,
    ui_font: Res<UiFont>,
    existing: Query<Entity, With<BoardGfx>>,
    active: Option<Res<ActiveLevel>>,
    editor: Res<Editor>,
) {
    let Some(active) = active else { return };
    let font = ui_font.0.clone();
    for e in &existing {
        commands.entity(e).despawn();
    }
    for cell in &active.level.buildable {
        let mut entity = commands.spawn((
            Sprite::from_color(col_grid(), Vec2::splat(CELL - 4.0)),
            Transform::from_translation(cell_world(*cell).extend(0.0)),
        ));
        Tag::Board.apply(&mut entity);
        // Cell index, top-left corner: errors and the switch panel talk in
        // coordinates — make them locatable on the board.
        let corner = cell_world(*cell) + Vec2::new(-CELL / 2.0 + 5.0, CELL / 2.0 - 5.0);
        let mut entity = commands.spawn((
            Text2d::new(format!("({},{})", cell.x, cell.y)),
            TextFont {
                font: font.clone(),
                font_size: 9.0,
                ..default()
            },
            TextColor(col_cell_index()),
            bevy::sprite::Anchor::TOP_LEFT,
            Transform::from_translation(corner.extend(0.5)),
        ));
        Tag::Board.apply(&mut entity);
    }
    draw_stations(&mut commands, &font, &active.level, Tag::Board);
    draw_layout(
        &mut commands,
        &font,
        &active.level.fixed,
        col_fixed(),
        Tag::Board,
    );
    draw_layout(&mut commands, &font, &editor.layout, col_player(), Tag::Board);
}
