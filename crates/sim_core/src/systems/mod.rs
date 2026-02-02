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
    use bevy_ecs::prelude::World;

    use crate::clock::{EventKind, EventSubject, SimulationClock, ONE_SEC_MS};
    use crate::ecs::{Driver, DriverState, Position, Rider, RiderState, Trip, TripState};
    use crate::runner::{run_until_empty, simulation_schedule};
    use crate::telemetry::SimTelemetry;

    #[test]
    fn simulates_one_ride_end_to_end() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(SimTelemetry::default());

        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");

        let rider_entity = world
            .spawn((
                Rider {
                    state: RiderState::Requesting,
                    matched_driver: None,
                    destination: None,
                    requested_at: None,
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
            .schedule_at_secs(1, EventKind::RequestInbound, Some(EventSubject::Rider(rider_entity)));

        let mut schedule = simulation_schedule();
        let steps = run_until_empty(&mut world, &mut schedule, 1000);
        assert!(steps < 1000, "runner did not converge");

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
        assert_eq!(trip.pickup, cell);
        // When destination is None, dropoff is a neighbor of pickup
        assert_ne!(trip.dropoff, trip.pickup, "dropoff should differ from pickup when defaulted");
        assert_eq!(rider.state, RiderState::Completed);
        assert_eq!(rider.matched_driver, None);
        assert_eq!(driver.state, DriverState::Idle);
        assert_eq!(driver.matched_rider, None);

        let telemetry = world.resource::<SimTelemetry>();
        assert_eq!(telemetry.completed_trips.len(), 1);
        let record = &telemetry.completed_trips[0];
        assert_eq!(record.rider_entity, rider_entity);
        assert_eq!(record.driver_entity, driver_entity);
        assert_eq!(record.trip_entity, trip_entity);
        assert!(record.completed_at >= ONE_SEC_MS, "completed_at should be in ms (>= 1s)");
        assert!(record.requested_at <= record.matched_at);
        assert!(record.matched_at <= record.pickup_at);
        assert!(record.pickup_at <= record.completed_at);
        assert_eq!(record.time_to_match(), record.matched_at - record.requested_at);
        assert_eq!(record.time_to_pickup(), record.pickup_at - record.matched_at);
        assert_eq!(record.trip_duration(), record.completed_at - record.pickup_at);
    }

    #[test]
    fn simulates_two_concurrent_rides_end_to_end() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(SimTelemetry::default());

        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");

        let rider1 = world
            .spawn((
                Rider {
                    state: RiderState::Requesting,
                    matched_driver: None,
                    destination: None,
                    requested_at: None,
                },
                Position(cell),
            ))
            .id();
        let rider2 = world
            .spawn((
                Rider {
                    state: RiderState::Requesting,
                    matched_driver: None,
                    destination: None,
                    requested_at: None,
                },
                Position(cell),
            ))
            .id();
        let driver1 = world
            .spawn((
                Driver {
                    state: DriverState::Idle,
                    matched_rider: None,
                },
                Position(cell),
            ))
            .id();
        let driver2 = world
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
            .schedule_at_secs(1, EventKind::RequestInbound, Some(EventSubject::Rider(rider1)));
        world
            .resource_mut::<SimulationClock>()
            .schedule_at_secs(2, EventKind::RequestInbound, Some(EventSubject::Rider(rider2)));

        let mut schedule = simulation_schedule();
        let steps = run_until_empty(&mut world, &mut schedule, 1000);
        assert!(steps < 1000, "runner did not converge");

        let trips: Vec<_> = world
            .query::<(bevy_ecs::prelude::Entity, &Trip)>()
            .iter(&world)
            .collect();
        assert_eq!(trips.len(), 2, "expected two completed trips");
        for (_trip_entity, trip) in &trips {
            assert_eq!(trip.state, TripState::Completed);
        }

        let rider1_state = world.entity(rider1).get::<Rider>().expect("rider1").state;
        let rider2_state = world.entity(rider2).get::<Rider>().expect("rider2").state;
        assert_eq!(rider1_state, RiderState::Completed);
        assert_eq!(rider2_state, RiderState::Completed);

        let driver1_state = world.entity(driver1).get::<Driver>().expect("driver1").state;
        let driver2_state = world.entity(driver2).get::<Driver>().expect("driver2").state;
        assert_eq!(driver1_state, DriverState::Idle);
        assert_eq!(driver2_state, DriverState::Idle);

        let telemetry = world.resource::<SimTelemetry>();
        assert_eq!(telemetry.completed_trips.len(), 2);
        let riders_drivers: Vec<_> = telemetry
            .completed_trips
            .iter()
            .map(|r| (r.rider_entity, r.driver_entity))
            .collect();
        assert!(riders_drivers.contains(&(rider1, driver1)));
        assert!(riders_drivers.contains(&(rider2, driver2)));
        for record in &telemetry.completed_trips {
            assert!(record.completed_at >= ONE_SEC_MS);
        }
    }
}
