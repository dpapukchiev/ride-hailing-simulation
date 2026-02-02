pub mod request_inbound;
pub mod quote_accepted;
pub mod simple_matching;
pub mod match_accepted;
pub mod driver_decision;
pub mod movement;
pub mod trip_started;
pub mod trip_completed;

#[cfg(test)]
mod end_to_end_tests {
    use bevy_ecs::schedule::apply_deferred;
    use bevy_ecs::prelude::{Schedule, World};

    use crate::clock::{CurrentEvent, Event, EventKind, EventSubject, SimulationClock};
    use crate::ecs::{Driver, DriverState, Position, Rider, RiderState, Trip, TripState};
    use crate::systems::{
        driver_decision::driver_decision_system, match_accepted::match_accepted_system,
        movement::movement_system, quote_accepted::quote_accepted_system,
        request_inbound::request_inbound_system, simple_matching::simple_matching_system,
        trip_completed::trip_completed_system, trip_started::trip_started_system,
    };

    #[test]
    fn simulates_one_ride_end_to_end() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());

        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");

        let rider_entity = world
            .spawn((
                Rider {
                    state: RiderState::Requesting,
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

        world
            .resource_mut::<SimulationClock>()
            .schedule(Event {
                timestamp: 1,
                kind: EventKind::RequestInbound,
                subject: Some(EventSubject::Rider(rider_entity)),
            });

        let mut schedule = Schedule::default();
        schedule.add_systems((
            request_inbound_system,
            quote_accepted_system,
            simple_matching_system,
            match_accepted_system,
            driver_decision_system,
            movement_system,
            trip_started_system,
            trip_completed_system,
            apply_deferred,
        ));

        // Runner loop: advance clock externally, route event via CurrentEvent.
        let mut steps = 0usize;
        while !world.resource::<SimulationClock>().is_empty() {
            steps += 1;
            assert!(steps < 1000, "runner did not converge");

            let event = world
                .resource_mut::<SimulationClock>()
                .pop_next()
                .expect("event");
            world.insert_resource(CurrentEvent(event));
            schedule.run(&mut world);
        }

        let trip_entity = world
            .query::<bevy_ecs::prelude::Entity>()
            .iter(&world)
            .find(|entity| world.entity(*entity).contains::<Trip>())
            .expect("trip entity");
        let trip = world.entity(trip_entity).get::<Trip>().expect("trip");

        let rider = world
            .entity(rider_entity)
            .get::<Rider>()
            .expect("rider");
        let driver = world
            .entity(driver_entity)
            .get::<Driver>()
            .expect("driver");

        assert_eq!(trip.state, TripState::Completed);
        assert_eq!(trip.rider, rider_entity);
        assert_eq!(trip.driver, driver_entity);
        assert_eq!(rider.state, RiderState::Completed);
        assert_eq!(rider.matched_driver, None);
        assert_eq!(driver.state, DriverState::Idle);
        assert_eq!(driver.matched_rider, None);
    }
}
