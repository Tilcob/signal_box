//! Trains: timetable departures, movement along the graph (respecting red
//! signals and switches), arrival scoring, collision detection, run end.

use bevy::prelude::*;

use crate::core::{GameState, Tunables};

use super::graph::{Cargo, NodeKind, TrackGraph};
use super::signal::Signal;
use super::EndCause;

const TRAIN_Z: f32 = 10.0;
pub const TRAIN_SIZE: Vec2 = Vec2::new(30.0, 12.0);

#[derive(Component)]
pub struct Train {
    /// Node the current edge starts at (already passed).
    pub from: usize,
    /// Node the train is heading toward.
    pub to: usize,
    /// Distance travelled along the current edge, from `from`.
    pub dist: f32,
    pub cargo: Cargo,
}

/// One scheduled departure. `delay` is measured from the previous departure.
pub struct Departure {
    pub delay: f32,
    pub source: usize,
    pub cargo: Cargo,
}

#[derive(Resource)]
pub struct Timetable {
    pub entries: Vec<Departure>,
    /// Index of the next departure not yet spawned.
    pub next: usize,
    /// Seconds until that departure is due.
    pub countdown: f32,
}

impl Timetable {
    pub fn remaining(&self) -> usize {
        self.entries.len() - self.next
    }
}

#[derive(Resource, Default)]
pub struct Score {
    pub delivered: u32,
    pub misrouted: u32,
}

/// The fixed schedule of the prototype level. The two `delay: 0.0` pairs are
/// the intended crunch moments: simultaneous departures from both sources that
/// collide at the merge unless one train is held at a red signal.
pub fn fresh_timetable() -> Timetable {
    use Cargo::{Blue, Orange};
    const A: usize = 0;
    const B: usize = 1;
    let entries = vec![
        Departure { delay: 1.5, source: A, cargo: Orange },
        Departure { delay: 4.0, source: B, cargo: Blue },
        Departure { delay: 4.0, source: A, cargo: Blue },
        Departure { delay: 3.0, source: B, cargo: Orange },
        Departure { delay: 5.0, source: A, cargo: Orange },
        Departure { delay: 0.0, source: B, cargo: Orange },
        Departure { delay: 5.0, source: B, cargo: Blue },
        Departure { delay: 2.5, source: A, cargo: Blue },
        Departure { delay: 4.0, source: A, cargo: Orange },
        Departure { delay: 0.0, source: B, cargo: Orange },
        Departure { delay: 4.0, source: B, cargo: Blue },
        Departure { delay: 2.0, source: A, cargo: Blue },
    ];
    let countdown = entries[0].delay;
    Timetable {
        entries,
        next: 0,
        countdown,
    }
}

pub fn spawn_departures(
    mut commands: Commands,
    mut timetable: ResMut<Timetable>,
    graph: Res<TrackGraph>,
    tunables: Res<Tunables>,
    time: Res<Time>,
    trains: Query<&Transform, With<Train>>,
) {
    timetable.countdown -= time.delta_secs();

    while timetable.next < timetable.entries.len() && timetable.countdown <= 0.0 {
        let departure = &timetable.entries[timetable.next];
        let source_pos = graph.pos(departure.source);

        // Hold the departure while the previous train still blocks the source.
        let blocked = trains.iter().any(|t| {
            t.translation.truncate().distance(source_pos) < tunables.spawn_clearance
        });
        if blocked {
            timetable.countdown = 0.25;
            return;
        }

        let to = graph
            .next_node(departure.source, departure.source)
            .expect("source node must have a continuation");
        let dir = (graph.pos(to) - source_pos).normalize();

        commands.spawn((
            Train {
                from: departure.source,
                to,
                dist: 0.0,
                cargo: departure.cargo,
            },
            Sprite::from_color(departure.cargo.color(), TRAIN_SIZE),
            Transform::from_translation(source_pos.extend(TRAIN_Z))
                .with_rotation(Quat::from_rotation_z(dir.to_angle())),
            Name::new(format!("Train {}", timetable.next)),
        ));

        timetable.next += 1;
        if timetable.next < timetable.entries.len() {
            let delay = timetable.entries[timetable.next].delay;
            timetable.countdown += delay;
        }
    }
}

pub fn move_trains(
    mut commands: Commands,
    mut trains: Query<(Entity, &mut Train, &mut Transform)>,
    graph: Res<TrackGraph>,
    signals: Query<&Signal>,
    tunables: Res<Tunables>,
    time: Res<Time>,
    mut score: ResMut<Score>,
) {
    for (entity, mut train, mut transform) in &mut trains {
        let mut remaining = tunables.train_speed * time.delta_secs();
        let mut alive = true;

        while remaining > 0.0001 {
            let len = graph.edge_len(train.from, train.to);

            // Nearest red signal at or ahead of the train on this directed
            // edge caps how far it may advance (0 if already standing at it).
            let mut limit = len - train.dist;
            for signal in &signals {
                if signal.from == train.from
                    && signal.to == train.to
                    && !signal.green
                    && signal.dist >= train.dist - 0.01
                {
                    limit = limit.min((signal.dist - train.dist).max(0.0));
                }
            }

            let step = remaining.min(limit);
            train.dist += step;
            remaining -= step;

            // Stopped short of the node (red signal): done for this frame.
            if train.dist < len - 0.0001 {
                break;
            }

            match graph.nodes[train.to].kind {
                NodeKind::Sink(accepts) => {
                    if accepts == train.cargo {
                        score.delivered += 1;
                    } else {
                        score.misrouted += 1;
                    }
                    commands.entity(entity).despawn();
                    alive = false;
                    break;
                }
                _ => match graph.next_node(train.from, train.to) {
                    Some(next) => {
                        train.from = train.to;
                        train.to = next;
                        train.dist = 0.0;
                    }
                    None => {
                        warn!("train reached dead end at node {}", train.to);
                        commands.entity(entity).despawn();
                        alive = false;
                        break;
                    }
                },
            }
        }

        if alive {
            let (a, b) = (graph.pos(train.from), graph.pos(train.to));
            let len = graph.edge_len(train.from, train.to);
            let pos = a.lerp(b, train.dist / len);
            transform.translation = pos.extend(TRAIN_Z);
            transform.rotation = Quat::from_rotation_z((b - a).to_angle());
        }
    }
}

pub fn detect_collisions(
    mut trains: Query<(&Transform, &mut Sprite), With<Train>>,
    tunables: Res<Tunables>,
    mut commands: Commands,
    mut next: ResMut<NextState<GameState>>,
) {
    let positions: Vec<Vec2> = trains
        .iter()
        .map(|(t, _)| t.translation.truncate())
        .collect();

    let mut crashed: Vec<usize> = Vec::new();
    for i in 0..positions.len() {
        for j in (i + 1)..positions.len() {
            if positions[i].distance(positions[j]) < tunables.collision_distance {
                crashed.push(i);
                crashed.push(j);
            }
        }
    }
    if crashed.is_empty() {
        return;
    }

    // Mark the wrecks dark red; they stay frozen on screen in the end state.
    for (index, (_, mut sprite)) in trains.iter_mut().enumerate() {
        if crashed.contains(&index) {
            sprite.color = Color::srgb(0.75, 0.10, 0.10);
        }
    }
    commands.insert_resource(EndCause::Crash);
    next.set(GameState::Ended);
}

pub fn check_completion(
    timetable: Res<Timetable>,
    trains: Query<(), With<Train>>,
    mut commands: Commands,
    mut next: ResMut<NextState<GameState>>,
) {
    if timetable.remaining() == 0 && trains.is_empty() {
        commands.insert_resource(EndCause::Completed);
        next.set(GameState::Ended);
    }
}
