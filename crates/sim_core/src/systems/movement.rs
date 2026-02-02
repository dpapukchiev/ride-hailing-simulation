use bevy_ecs::prelude::{Entity, ParamSet, Query, ResMut, With};

use crate::clock::{Event, EventKind, SimulationClock};
use crate::ecs::{Driver, DriverState, Position, Rider};

fn eta_ticks(distance: i32) -> u64 {
    if distance <= 0 {
        1
    } else {
        distance as u64
    }
}

pub fn movement_system(
    mut clock: ResMut<SimulationClock>,
    mut queries: ParamSet<(
        Query<(Entity, &mut Driver, &mut Position)>,
        Query<(Entity, &Position), With<Rider>>,
    )>,
) {
    let event = match clock.pop_next() {
        Some(event) => event,
        None => return,
    };

    if event.kind != EventKind::MoveStep {
        return;
    }

    let mut any_en_route = false;
    let mut any_arrived = false;
    let mut min_remaining: Option<i32> = None;

    let rider_positions: Vec<(Entity, h3o::CellIndex)> = {
        let riders = queries.p1();
        riders.iter().map(|(entity, pos)| (entity, pos.0)).collect()
    };
    let mut drivers = queries.p0();

    for (_driver_entity, driver, mut driver_pos) in drivers.iter_mut() {
        if driver.state != DriverState::EnRoute {
            continue;
        }
        let Some(rider_entity) = driver.matched_rider else {
            continue;
        };
        let Some((_, rider_pos)) = rider_positions
            .iter()
            .find(|(entity, _)| *entity == rider_entity)
        else {
            continue;
        };

        let distance = driver_pos
            .0
            .grid_distance(*rider_pos)
            .unwrap_or(0);
        if distance <= 0 {
            any_arrived = true;
            continue;
        }

        if let Ok(path) = driver_pos.0.grid_path_cells(*rider_pos) {
            let mut iter = path.filter_map(|cell| cell.ok());
            let _current = iter.next();
            if let Some(next_cell) = iter.next() {
                driver_pos.0 = next_cell;
            }
        }

        let remaining = driver_pos
            .0
            .grid_distance(*rider_pos)
            .unwrap_or(0);
        if remaining == 0 {
            any_arrived = true;
        } else {
            any_en_route = true;
            min_remaining = Some(match min_remaining {
                Some(current) => current.min(remaining),
                None => remaining,
            });
        }
    }

    if any_arrived {
        let next_timestamp = clock.now() + eta_ticks(0);
        clock.schedule(Event {
            timestamp: next_timestamp,
            kind: EventKind::TripStarted,
        });
    }

    if any_en_route {
        let remaining = min_remaining.unwrap_or(1);
        let next_timestamp = clock.now() + eta_ticks(remaining);
        clock.schedule(Event {
            timestamp: next_timestamp,
            kind: EventKind::MoveStep,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};
    use crate::ecs::RiderState;

    #[test]
    fn movement_steps_toward_rider_and_schedules_trip_start() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());

        let origin = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
        let neighbor = origin
            .grid_disk::<Vec<_>>(1)
            .into_iter()
            .find(|cell| *cell != origin)
            .expect("neighbor");

        let rider_entity = world
            .spawn((
                Rider {
                    state: RiderState::Waiting,
                    matched_driver: None,
                },
                Position(neighbor),
            ))
            .id();
        let driver_entity = world
            .spawn((
                Driver {
                    state: DriverState::EnRoute,
                    matched_rider: Some(rider_entity),
                },
                Position(origin),
            ))
            .id();

        world
            .resource_mut::<SimulationClock>()
            .schedule(Event {
                timestamp: 1,
                kind: EventKind::MoveStep,
            });

        let mut schedule = Schedule::default();
        schedule.add_systems(movement_system);
        schedule.run(&mut world);

        let driver_position = {
            let pos = world.query::<&Position>().get(&world, driver_entity).expect("pos");
            pos.0
        };
        assert_eq!(driver_position, neighbor);

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("trip started event");
        assert_eq!(next_event.kind, EventKind::TripStarted);
        assert_eq!(next_event.timestamp, 2);
    }

    #[test]
    fn eta_ticks_scales_with_distance() {
        assert_eq!(eta_ticks(0), 1);
        assert_eq!(eta_ticks(1), 1);
        assert_eq!(eta_ticks(3), 3);
    }
}
