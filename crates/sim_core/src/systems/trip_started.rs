use bevy_ecs::prelude::{Entity, Query, ResMut};

use crate::clock::{Event, EventKind, SimulationClock};
use crate::ecs::{Driver, DriverState, Position, Rider, RiderState};

pub fn trip_started_system(
    mut clock: ResMut<SimulationClock>,
    mut riders: Query<(Entity, &mut Rider, &Position)>,
    mut drivers: Query<(Entity, &mut Driver, &Position)>,
) {
    let event = match clock.pop_next() {
        Some(event) => event,
        None => return,
    };

    if event.kind != EventKind::TripStarted {
        return;
    }

    let mut ready_pairs: Vec<(Entity, Entity)> = Vec::new();

    for (rider_entity, rider, rider_pos) in riders.iter() {
        if rider.state != RiderState::Waiting {
            continue;
        }
        let Some(driver_entity) = rider.matched_driver else {
            continue;
        };
        let Ok((_driver_entity, driver, driver_pos)) = drivers.get(driver_entity) else {
            continue;
        };
        if driver.state == DriverState::EnRoute && driver_pos.0 == rider_pos.0 {
            ready_pairs.push((rider_entity, driver_entity));
        }
    }

    for (rider_entity, driver_entity) in &ready_pairs {
        if let Ok((_entity, mut rider, _)) = riders.get_mut(*rider_entity) {
            rider.state = RiderState::InTransit;
        }
        if let Ok((_entity, mut driver, _)) = drivers.get_mut(*driver_entity) {
            driver.state = DriverState::OnTrip;
        }
    }

    if !ready_pairs.is_empty() {
        let next_timestamp = clock.now() + 1;
        clock.schedule(Event {
            timestamp: next_timestamp,
            kind: EventKind::TripCompleted,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    #[test]
    fn trip_started_transitions_and_schedules_completion() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
        let rider_entity = world
            .spawn((
                Rider {
                    state: RiderState::Waiting,
                    matched_driver: None,
                },
                Position(cell),
            ))
            .id();
        let driver_entity = world
            .spawn((
                Driver {
                    state: DriverState::EnRoute,
                    matched_rider: Some(rider_entity),
                },
                Position(cell),
            ))
            .id();

        {
            let mut rider_entity_mut = world.entity_mut(rider_entity);
            let mut rider = rider_entity_mut.get_mut::<Rider>().expect("rider");
            rider.matched_driver = Some(driver_entity);
        }

        world
            .resource_mut::<SimulationClock>()
            .schedule(Event {
                timestamp: 3,
                kind: EventKind::TripStarted,
            });

        let mut schedule = Schedule::default();
        schedule.add_systems(trip_started_system);
        schedule.run(&mut world);

        let rider_state = {
            let rider = world.query::<&Rider>().single(&world);
            rider.state
        };
        let driver_state = {
            let driver = world.query::<&Driver>().single(&world);
            driver.state
        };

        assert_eq!(rider_state, RiderState::InTransit);
        assert_eq!(driver_state, DriverState::OnTrip);

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("completion event");
        assert_eq!(next_event.kind, EventKind::TripCompleted);
        assert_eq!(next_event.timestamp, 4);
    }
}
