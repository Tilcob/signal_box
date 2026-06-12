//! Board rendering — the Pult look (GDD §10): near-black desk, tracks as
//! narrow light bands, state shown by glow color AND shape (accessibility,
//! GDD §9: occupied bands are wider, reservations narrower; chain signals
//! are diamonds, block signals squares — never color alone).
//!
//! Strategy (M1-minimal): retained sprites, fully rebuilt when the build
//! changes (edit) or every frame (run — states change constantly, boards
//! are small). No lyon yet: every stub is a straight segment in the stub
//! model, so rotated quads cover the whole look. Revisit when curves or
//! round caps are wanted (GDD §12.2 note).

use bevy::prelude::*;
use std::collections::BTreeSet;
use stellwerk_sim::Sim;
use stellwerk_sim::graph::Next;
use stellwerk_sim::grid::{Cell, Dir8, Point};
use stellwerk_sim::layout::{Layout, SignalKind};
use stellwerk_sim::level::Level;
use stellwerk_sim::units::BlockId;

use crate::run::RunCtl;
use crate::state::{ActiveLevel, Editor, GameState};

pub const CELL: f32 = 96.0;

pub fn point_world(p: Point) -> Vec2 {
    Vec2::new(p.x as f32, p.y as f32) * (CELL / 2.0)
}

pub fn cell_world(c: Cell) -> Vec2 {
    point_world(c.center_point())
}

pub fn connector_world(c: Cell, d: Dir8) -> Vec2 {
    point_world(c.connector_point(d))
}

/// World position → cell under the cursor.
pub fn world_cell(pos: Vec2) -> Cell {
    Cell {
        x: (pos.x / CELL).floor() as i32,
        y: (pos.y / CELL).floor() as i32,
    }
}

/// Connector of `cell` nearest to a world position.
pub fn nearest_connector(cell: Cell, pos: Vec2) -> Dir8 {
    *Dir8::ALL
        .iter()
        .min_by(|a, b| {
            let da = connector_world(cell, **a).distance_squared(pos);
            let db = connector_world(cell, **b).distance_squared(pos);
            da.total_cmp(&db)
        })
        .expect("ALL is non-empty")
}

// --- Palette ---------------------------------------------------------------

pub fn col_grid() -> Color {
    Color::srgb(0.030, 0.035, 0.045)
}
pub fn col_fixed() -> Color {
    Color::srgb(0.16, 0.19, 0.24)
}
pub fn col_player() -> Color {
    Color::srgb(0.30, 0.34, 0.42)
}
pub fn col_switch_active() -> Color {
    Color::LinearRgba(LinearRgba::rgb(1.6, 1.2, 0.25))
}
pub fn col_switch_inactive() -> Color {
    Color::srgb(0.10, 0.10, 0.08)
}
pub fn col_signal_green() -> Color {
    Color::LinearRgba(LinearRgba::rgb(0.25, 2.2, 0.5))
}
pub fn col_signal_red() -> Color {
    Color::LinearRgba(LinearRgba::rgb(2.6, 0.25, 0.2))
}
pub fn col_occupied() -> Color {
    Color::LinearRgba(LinearRgba::rgb(1.5, 0.30, 0.22))
}
pub fn col_reserved() -> Color {
    Color::LinearRgba(LinearRgba::rgb(1.3, 0.95, 0.20))
}
pub fn col_train() -> Color {
    Color::LinearRgba(LinearRgba::rgb(2.4, 1.9, 1.1))
}
pub fn col_head() -> Color {
    Color::LinearRgba(LinearRgba::rgb(4.0, 3.2, 1.8))
}
pub fn col_label() -> Color {
    Color::srgb(0.55, 0.58, 0.65)
}

// --- Tags -------------------------------------------------------------------

/// Static board sprites (rebuilt on layout change / mode entry).
#[derive(Component)]
pub struct BoardGfx;

/// Per-frame run sprites (trains, lit bands, lamps).
#[derive(Component)]
pub struct LiveGfx;

pub struct BoardPlugin;

impl Plugin for BoardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Edit), rebuild_edit_board)
            .add_systems(
                Update,
                rebuild_edit_board.run_if(in_state(GameState::Edit).and(
                    resource_changed::<Editor>.or(resource_exists_and_changed::<ActiveLevel>),
                )),
            )
            .add_systems(OnExit(GameState::Edit), despawn_all::<BoardGfx>)
            .add_systems(
                Update,
                draw_run_board.run_if(in_state(GameState::Run).or(in_state(GameState::Result))),
            )
            .add_systems(OnExit(GameState::Result), despawn_all::<LiveGfx>);
    }
}

fn despawn_all<C: Component>(mut commands: Commands, q: Query<Entity, With<C>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

fn band(commands: &mut Commands, a: Vec2, b: Vec2, width: f32, color: Color, z: f32, tag: Tag) {
    let mid = (a + b) / 2.0;
    let delta = b - a;
    let entity = commands.spawn((
        Sprite::from_color(color, Vec2::new(delta.length().max(1.0), width)),
        Transform::from_translation(mid.extend(z))
            .with_rotation(Quat::from_rotation_z(delta.y.atan2(delta.x))),
    ));
    tag.apply(entity);
}

fn lamp(
    commands: &mut Commands,
    pos: Vec2,
    size: f32,
    color: Color,
    diamond: bool,
    z: f32,
    tag: Tag,
) {
    let rot = if diamond {
        Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)
    } else {
        Quat::IDENTITY
    };
    let entity = commands.spawn((
        Sprite::from_color(color, Vec2::splat(size)),
        Transform::from_translation(pos.extend(z)).with_rotation(rot),
    ));
    tag.apply(entity);
}

fn label(commands: &mut Commands, pos: Vec2, text: String, size: f32, color: Color, tag: Tag) {
    let entity = commands.spawn((
        Text2d::new(text),
        TextFont::from_font_size(size),
        TextColor(color),
        Transform::from_translation(pos.extend(6.0)),
    ));
    tag.apply(entity);
}

/// Which marker the spawned entity gets.
#[derive(Clone, Copy)]
enum Tag {
    Board,
    Live,
}

impl Tag {
    fn apply(self, mut entity: bevy::ecs::system::EntityCommands) {
        match self {
            Tag::Board => entity.insert(BoardGfx),
            Tag::Live => entity.insert(LiveGfx),
        };
    }
}

/// Signal lamp position: at the connector, offset perpendicular to the stub.
fn signal_pos(cell: Cell, at: Dir8) -> Vec2 {
    let connector = connector_world(cell, at);
    let inward = (cell_world(cell) - connector).normalize_or_zero();
    connector + inward.perp() * 16.0
}

fn draw_layout(commands: &mut Commands, layout: &Layout, color: Color, tag: Tag) {
    for piece in &layout.pieces {
        let center = cell_world(piece.cell);
        band(
            commands,
            connector_world(piece.cell, piece.a),
            center,
            7.0,
            color,
            2.0,
            tag,
        );
        band(
            commands,
            connector_world(piece.cell, piece.b),
            center,
            7.0,
            color,
            2.0,
            tag,
        );
    }
    for switch in &layout.switches {
        let center = cell_world(switch.cell);
        band(
            commands,
            connector_world(switch.cell, switch.stem),
            center,
            7.0,
            color,
            2.0,
            tag,
        );
        for (i, branch) in switch.branches.iter().enumerate() {
            let branch_color = if i as u8 == switch.default_branch {
                col_switch_active()
            } else {
                col_switch_inactive()
            };
            band(
                commands,
                connector_world(switch.cell, *branch),
                center,
                7.0,
                branch_color,
                3.0,
                tag,
            );
        }
        lamp(commands, center, 12.0, col_switch_active(), true, 4.0, tag);
        if !switch.rules.is_empty() {
            label(
                commands,
                center + Vec2::new(0.0, 18.0),
                format!("{}R", switch.rules.len()),
                12.0,
                col_label(),
                tag,
            );
        }
    }
    for signal in &layout.signals {
        lamp(
            commands,
            signal_pos(signal.cell, signal.at),
            14.0,
            col_signal_green(),
            matches!(signal.kind, SignalKind::Chain),
            5.0,
            tag,
        );
    }
}

fn draw_stations(commands: &mut Commands, level: &Level, tag: Tag) {
    for source in &level.sources {
        let connector = connector_world(source.cell, source.dir);
        let outward = (connector - cell_world(source.cell)).normalize_or_zero();
        band(
            commands,
            connector,
            connector + outward * 30.0,
            10.0,
            col_fixed(),
            2.0,
            tag,
        );
        label(
            commands,
            connector + outward * 58.0,
            format!("Q{}", source.id.0),
            13.0,
            col_label(),
            tag,
        );
    }
    for sink in &level.sinks {
        let connector = connector_world(sink.cell, sink.dir);
        let outward = (connector - cell_world(sink.cell)).normalize_or_zero();
        band(
            commands,
            connector,
            connector + outward * 30.0,
            10.0,
            col_fixed(),
            2.0,
            tag,
        );
        label(
            commands,
            connector + outward * 26.0 + Vec2::new(0.0, 26.0),
            sink.label.clone(),
            14.0,
            col_label(),
            tag,
        );
    }
}

fn rebuild_edit_board(
    mut commands: Commands,
    existing: Query<Entity, With<BoardGfx>>,
    active: Option<Res<ActiveLevel>>,
    editor: Res<Editor>,
) {
    let Some(active) = active else { return };
    for e in &existing {
        commands.entity(e).despawn();
    }
    for cell in &active.level.buildable {
        let entity = commands.spawn((
            Sprite::from_color(col_grid(), Vec2::splat(CELL - 4.0)),
            Transform::from_translation(cell_world(*cell).extend(0.0)),
        ));
        Tag::Board.apply(entity);
    }
    draw_stations(&mut commands, &active.level, Tag::Board);
    draw_layout(&mut commands, &active.level.fixed, col_fixed(), Tag::Board);
    draw_layout(&mut commands, &editor.layout, col_player(), Tag::Board);
}

/// Run/Result rendering: graph bands with block states, signal lamps, train
/// bodies and the interpolated head light.
fn draw_run_board(
    mut commands: Commands,
    existing: Query<Entity, With<LiveGfx>>,
    active: Option<Res<ActiveLevel>>,
    ctl: Option<Res<RunCtl>>,
) {
    let (Some(active), Some(ctl)) = (active, ctl) else {
        return;
    };
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
    draw_stations(&mut commands, &active.level, Tag::Live);

    for (i, edge) in graph.edges.iter().enumerate() {
        if edge.opposite.0 < i as u32 {
            continue; // canonical direction only
        }
        let block = graph
            .blocks
            .block_of(stellwerk_sim::units::EdgeId(i as u32));
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
    let merged = active.level.fixed.merged(&ctl.layout);
    for signal in &merged.signals {
        let lit = signal_display_state(sim, signal.cell, signal.at, &occupied, &reserved);
        lamp(
            &mut commands,
            signal_pos(signal.cell, signal.at),
            14.0,
            if lit {
                col_signal_green()
            } else {
                col_signal_red()
            },
            matches!(signal.kind, SignalKind::Chain),
            5.0,
            Tag::Live,
        );
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
    cell: Cell,
    at: Dir8,
    occupied: &BTreeSet<BlockId>,
    reserved: &BTreeSet<BlockId>,
) -> bool {
    let graph = sim.graph();
    // Find the gated edge: cell center → connector.
    let center = cell.center_point();
    let connector = cell.connector_point(at);
    let Some((index, edge)) =
        graph.edges.iter().enumerate().find(|(_, e)| {
            graph.node(e.from).point == center && graph.node(e.to).point == connector
        })
    else {
        return true;
    };
    let _ = index;
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
