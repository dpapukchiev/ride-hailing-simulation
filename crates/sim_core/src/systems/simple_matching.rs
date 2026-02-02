use bevy_ecs::prelude::{Entity, Query, ResMut};

use crate::clock::{Event, EventKind, SimulationClock};
use crate::ecs::{Driver, DriverState, Position, Rider, RiderState};

pub fn simple_matching_system(
    mut clock: ResMut<SimulationClock>,
    mut riders: Query<(Entity, &mut Rider, &Position)>,
    mut drivers: Query<(Entity, &mut Driver, &Position)>,
) {
    let rider = riders
        .iter()
        .find(|(_, rider, _)| rider.state == RiderState::Waiting)
        .map(|(entity, _, position)| (entity, position.0));
    let rider_position = rider.map(|(_, position)| position);
    let driver_entity = drivers
        .iter()
        .find(|(_, driver, position)| {
            driver.state == DriverState::Idle
                && rider_position.map(|rider_pos| rider_pos == position.0).unwrap_or(false)
        })
        .map(|(entity, _, _)| entity);

    let (Some((rider_entity, _)), Some(driver_entity)) = (rider, driver_entity) else {
        return;
    };

    if let Ok((_, mut rider, _)) = riders.get_mut(rider_entity) {
        rider.state = RiderState::Waiting;
        rider.matched_driver = Some(driver_entity);
    }
    if let Ok((_, mut driver, _)) = drivers.get_mut(driver_entity) {
        driver.state = DriverState::Evaluating;
        driver.matched_rider = Some(rider_entity);
    }

    let next_timestamp = clock.now() + 1;
    clock.schedule(Event {
        timestamp: next_timestamp,
        kind: EventKind::MatchAccepted,
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
    }
}
