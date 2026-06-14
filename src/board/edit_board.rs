//! The static edit-mode board: buildable grid with cell indices, stations,
//! fixed and player layout.

use bevy::prelude::*;
use std::collections::BTreeMap;
use stellwerk_sim::grid::{Cell, Dir8};
use stellwerk_sim::layout::Layout;
use stellwerk_sim::level::Level;
use stellwerk_sim::units::EdgeId;

use super::draw::{BlockColors, BoardGfx, Tag, draw_layout, draw_stations};
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
    // Block partition, so the player sees where their signals cut the net
    // while building — same hues the run board uses. Best-effort: an invalid
    // build has no graph, and both passes fall back to their flat colour.
    let blocks = block_colors(&active.level, &editor.layout);
    draw_layout(
        &mut commands,
        &font,
        &active.level.fixed,
        col_fixed(),
        blocks.as_ref(),
        Tag::Board,
    );
    draw_layout(
        &mut commands,
        &font,
        &editor.layout,
        col_player(),
        blocks.as_ref(),
        Tag::Board,
    );
}

/// Maps every stub `(cell, connector)` of the merged layout to its block, by
/// building the live graph and resolving the inward stub edge per stub. `None`
/// when the layout does not validate (the graph cannot be built yet).
fn block_colors(level: &Level, player: &Layout) -> Option<BlockColors> {
    let graph = stellwerk_sim::graph::build(level, player).ok()?;
    let mut by_point = BTreeMap::new();
    for (i, edge) in graph.edges.iter().enumerate() {
        by_point.insert(
            (graph.node(edge.from).point, graph.node(edge.to).point),
            EdgeId(i as u32),
        );
    }
    let merged = level.fixed.merged(player);
    let mut stubs: Vec<(Cell, Dir8)> = Vec::new();
    for piece in &merged.pieces {
        stubs.push((piece.cell, piece.a));
        stubs.push((piece.cell, piece.b));
    }
    for switch in &merged.switches {
        stubs.push((switch.cell, switch.stem));
        stubs.push((switch.cell, switch.branches[0]));
        stubs.push((switch.cell, switch.branches[1]));
    }
    let mut out = BlockColors::new();
    for (cell, dir) in stubs {
        let key = (cell.connector_point(dir), cell.center_point());
        if let Some(&edge) = by_point.get(&key) {
            out.insert((cell, dir), graph.blocks.block_of(edge));
        }
    }
    Some(out)
}
