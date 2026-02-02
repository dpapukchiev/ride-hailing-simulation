use bevy_ecs::prelude::{Entity, ParamSet, Query, Res, ResMut, With};

use crate::clock::{CurrentEvent, Event, EventKind, EventSubject, SimulationClock};
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
    event: Res<CurrentEvent>,
    mut queries: ParamSet<(
        Query<(Entity, &mut Driver, &mut Position)>,
        Query<(Entity, &Position), With<Rider>>,
    )>,
) {
    if event.0.kind != EventKind::MoveStep {
        return;
    }

    let Some(EventSubject::Driver(driver_entity)) = event.0.subject else {
        return;
    };

    let rider_entity = {
        let drivers = queries.p0();
        let Ok((_entity, driver, _pos)) = drivers.get(driver_entity) else {
            return;
        };
        if driver.state != DriverState::EnRoute {
            return;
        }
        let Some(rider_entity) = driver.matched_rider else {
            return;
        };
        rider_entity
    };

    let rider_pos = {
        let riders = queries.p1();
        let Ok((_entity, pos)) = riders.get(rider_entity) else {
            return;
        };
        pos.0
    };

    let mut drivers = queries.p0();
    let Ok((_entity, _driver, mut driver_pos)) = drivers.get_mut(driver_entity) else {
        return;
    };

    let distance = driver_pos.0.grid_distance(rider_pos).unwrap_or(0);
    if distance <= 0 {
        let next_timestamp = clock.now() + eta_ticks(0);
        clock.schedule(Event {
            timestamp: next_timestamp,
            kind: EventKind::TripStarted,
            subject: Some(EventSubject::Driver(driver_entity)),
        });
        return;
    }

    if let Ok(path) = driver_pos.0.grid_path_cells(rider_pos) {
        let mut iter = path.filter_map(|cell| cell.ok());
        let _current = iter.next();
        if let Some(next_cell) = iter.next() {
            driver_pos.0 = next_cell;
        }
    }

    let remaining = driver_pos.0.grid_distance(rider_pos).unwrap_or(0);
    if remaining == 0 {
        let next_timestamp = clock.now() + eta_ticks(0);
        clock.schedule(Event {
            timestamp: next_timestamp,
            kind: EventKind::TripStarted,
            subject: Some(EventSubject::Driver(driver_entity)),
        });
    } else {
        let next_timestamp = clock.now() + eta_ticks(remaining);
        clock.schedule(Event {
            timestamp: next_timestamp,
            kind: EventKind::MoveStep,
            subject: Some(EventSubject::Driver(driver_entity)),
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
                subject: Some(EventSubject::Driver(driver_entity)),
            });
        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("move step event");
        world.insert_resource(CurrentEvent(event));

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
        assert_eq!(next_event.subject, Some(EventSubject::Driver(driver_entity)));
    }

    #[test]
    fn eta_ticks_scales_with_distance() {
        assert_eq!(eta_ticks(0), 1);
        assert_eq!(eta_ticks(1), 1);
        assert_eq!(eta_ticks(3), 3);
    }
}
