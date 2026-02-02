use bevy_ecs::prelude::{Entity, Query, Res, ResMut};

use crate::clock::{CurrentEvent, Event, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Driver, DriverState, Position, Rider, RiderState};

pub fn simple_matching_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    mut riders: Query<(Entity, &mut Rider, &Position)>,
    mut drivers: Query<(Entity, &mut Driver, &Position)>,
) {
    if event.0.kind != EventKind::TryMatch {
        return;
    }

    let Some(EventSubject::Rider(rider_entity)) = event.0.subject else {
        return;
    };

    let rider_position = {
        let Ok((_entity, rider, position)) = riders.get(rider_entity) else {
            return;
        };
        if rider.state != RiderState::Waiting {
            return;
        }
        position.0
    };

    let driver_entity = drivers
        .iter()
        .find(|(_, driver, position)| {
            driver.state == DriverState::Idle && position.0 == rider_position
        })
        .map(|(entity, _, _)| entity);
    let Some(driver_entity) = driver_entity else {
        return;
    };

    if let Ok((_entity, mut rider, _)) = riders.get_mut(rider_entity) {
        rider.matched_driver = Some(driver_entity);
    }
    if let Ok((_entity, mut driver, _)) = drivers.get_mut(driver_entity) {
        driver.state = DriverState::Evaluating;
        driver.matched_rider = Some(rider_entity);
    }

    let next_timestamp = clock.now() + 1;
    clock.schedule(Event {
        timestamp: next_timestamp,
        kind: EventKind::MatchAccepted,
        subject: Some(EventSubject::Driver(driver_entity)),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    #[test]
    fn matches_waiting_rider_to_idle_driver() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
        let rider_entity = world
            .spawn((
                Rider {
                    state: RiderState::Waiting,
                    matched_driver: None,
                    destination: None,
                },
                Position(cell),
            ))
            .id();
        let driver_entity = world
            .spawn((
                Driver {
                    state: DriverState::Idle,
                    matched_rider: None,
                },
                Position(cell),
            ))
            .id();

        world
            .resource_mut::<SimulationClock>()
            .schedule(Event {
                timestamp: 0,
                kind: EventKind::TryMatch,
                subject: Some(EventSubject::Rider(rider_entity)),
            });
        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("try match event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems(simple_matching_system);
        schedule.run(&mut world);

        let (rider_state, matched_driver) = {
            let rider = world.query::<&Rider>().single(&world);
            (rider.state, rider.matched_driver)
        };
        let (driver_state, matched_rider) = {
            let driver = world.query::<&Driver>().single(&world);
            (driver.state, driver.matched_rider)
        };

        assert_eq!(rider_state, RiderState::Waiting);
        assert_eq!(driver_state, DriverState::Evaluating);
        assert_eq!(matched_driver, Some(driver_entity));
        assert_eq!(matched_rider, Some(rider_entity));

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("trip started event");
        assert_eq!(next_event.kind, EventKind::MatchAccepted);
        assert_eq!(next_event.timestamp, 1);
        assert_eq!(next_event.subject, Some(EventSubject::Driver(driver_entity)));
    }
}
