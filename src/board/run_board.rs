//! Run/Result rendering: graph bands with block states, signal lamps, train
//! bodies and the interpolated head light.

use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use stellwerk_sim::Sim;
use stellwerk_sim::graph::Next;
use stellwerk_sim::grid::{Cell, Dir8, Point};
use stellwerk_sim::layout::SignalKind;
use stellwerk_sim::units::{BlockId, EdgeId};

use super::draw::{LiveGfx, Tag, band, draw_stations, lamp, label, signal_direction_tick, signal_pos};
use super::geometry::{CELL, cell_world, point_world};
use super::palette::*;
use crate::font::UiFont;
use crate::run::RunCtl;
use crate::state::ActiveLevel;

pub(super) fn draw_run_board(
    mut commands: Commands,
    ui_font: Res<UiFont>,
    existing: Query<Entity, With<LiveGfx>>,
    active: Option<Res<ActiveLevel>>,
    ctl: Option<Res<RunCtl>>,
) {
    let (Some(active), Some(ctl)) = (active, ctl) else {
        return;
    };
    let font = ui_font.0.clone();
    for e in &existing {
        commands.entity(e).despawn();
    }
    let sim = &ctl.sim;
    let graph = sim.graph();

    // Block states.
    let mut occupied: BTreeSet<BlockId> = BTreeSet::new();
    for train in sim.trains() {
        for (edge, lo, hi) in train.occupied(graph) {
            if hi > lo {
                occupied.insert(graph.blocks.block_of(edge));
            }
        }
        occupied.insert(graph.blocks.block_of(train.head_edge()));
    }
    let reserved: BTreeSet<BlockId> = sim.reservations().keys().copied().collect();

    for cell in &active.level.buildable {
        let entity = commands.spawn((
            Sprite::from_color(col_grid(), Vec2::splat(CELL - 4.0)),
            Transform::from_translation(cell_world(*cell).extend(0.0)),
        ));
        Tag::Live.apply(entity);
    }
    draw_stations(&mut commands, &font, &active.level, Tag::Live);

    for (i, edge) in graph.edges.iter().enumerate() {
        if edge.opposite.0 < i as u32 {
            continue; // canonical direction only
        }
        let block = graph.blocks.block_of(EdgeId(i as u32));
        // State by color AND width (accessibility: never color alone).
        let (color, width) = if occupied.contains(&block) {
            (col_occupied(), 9.0)
        } else if reserved.contains(&block) {
            (col_reserved(), 5.0)
        } else {
            (col_player(), 7.0)
        };
        band(
            &mut commands,
            point_world(graph.node(edge.from).point),
            point_world(graph.node(edge.to).point),
            width,
            color,
            2.0,
            Tag::Live,
        );
    }

    // Signal lamps with live state (display heuristic: red while the guarded
    // next block is occupied or reserved; switches judged by default branch).
    // (center, connector) → gated edge, built once per frame — a linear edge
    // scan per signal would be O(signals × edges).
    let mut edge_at: BTreeMap<(Point, Point), EdgeId> = BTreeMap::new();
    for (i, edge) in graph.edges.iter().enumerate() {
        edge_at.insert(
            (graph.node(edge.from).point, graph.node(edge.to).point),
            EdgeId(i as u32),
        );
    }
    let merged = active.level.fixed.merged(&ctl.layout);
    for signal in &merged.signals {
        let lit = signal_display_state(sim, &edge_at, signal.cell, signal.at, &occupied, &reserved);
        let color = if lit {
            col_signal_green()
        } else {
            col_signal_red()
        };
        lamp(
            &mut commands,
            signal_pos(signal.cell, signal.at),
            14.0,
            color,
            matches!(signal.kind, SignalKind::Chain),
            5.0,
            Tag::Live,
        );
        signal_direction_tick(&mut commands, signal.cell, signal.at, color, Tag::Live);
    }

    // Trains: bright body bands + interpolated head light + number.
    for train in sim.trains() {
        for (edge, lo, hi) in train.occupied(graph) {
            let data = graph.edge(edge);
            let a = point_world(graph.node(data.from).point);
            let b = point_world(graph.node(data.to).point);
            let len = data.len.0 as f32;
            band(
                &mut commands,
                a.lerp(b, lo.0 as f32 / len),
                a.lerp(b, hi.0 as f32 / len),
                9.0,
                col_train(),
                10.0,
                Tag::Live,
            );
        }
        let head = ctl.interpolated_head(train.id);
        lamp(
            &mut commands,
            head,
            13.0,
            col_head(),
            false,
            11.0,
            Tag::Live,
        );
        label(
            &mut commands,
            &font,
            head + Vec2::new(0.0, 20.0),
            format!("{}", train.id.0),
            13.0,
            col_label(),
            Tag::Live,
        );
    }
}

fn signal_display_state(
    sim: &Sim,
    edge_at: &BTreeMap<(Point, Point), EdgeId>,
    cell: Cell,
    at: Dir8,
    occupied: &BTreeSet<BlockId>,
    reserved: &BTreeSet<BlockId>,
) -> bool {
    let graph = sim.graph();
    // The gated edge runs cell center → anchored connector.
    let Some(&gated) = edge_at.get(&(cell.center_point(), cell.connector_point(at))) else {
        return true;
    };
    let edge = graph.edge(gated);
    let next = match edge.next {
        Next::Fixed(e) => e,
        Next::SwitchChoice { switch } => {
            let sw = &graph.switches[switch as usize];
            sw.branch_out[sw.default_branch as usize]
        }
        Next::DeadEnd => return true,
    };
    let block = graph.blocks.block_of(next);
    !(occupied.contains(&block) || reserved.contains(&block))
}
