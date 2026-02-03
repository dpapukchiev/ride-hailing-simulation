pub mod request_inbound;
pub mod quote_accepted;
pub mod simple_matching;
pub mod match_accepted;
pub mod driver_decision;
pub mod movement;
pub mod pickup_eta_updated;
pub mod trip_started;
pub mod trip_completed;
pub mod telemetry_snapshot;
pub mod rider_cancel;

#[cfg(test)]
mod end_to_end_tests {
    use bevy_ecs::prelude::World;

    use crate::clock::{EventKind, SimulationClock, ONE_SEC_MS};
    use crate::ecs::{Driver, DriverState, Position, Trip, TripState};
    use crate::runner::{run_until_empty, simulation_schedule};
    use crate::scenario::PendingRider;
    use crate::scenario::PendingRiders;
    use crate::speed::SpeedModel;
    use crate::telemetry::{SimSnapshotConfig, SimSnapshots, SimTelemetry};

    #[test]
    fn simulates_one_ride_end_to_end() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(SimTelemetry::default());
        world.insert_resource(SimSnapshotConfig::default());
        world.insert_resource(SimSnapshots::default());
        world.insert_resource(SpeedModel::with_range(Some(1), 40.0, 40.0));

        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
        // Pick a neighbor cell as destination
        let destination = cell
            .grid_disk::<Vec<_>>(1)
            .into_iter()
            .find(|c| *c != cell)
            .unwrap_or(cell);

        // Add pending rider
        let mut pending_riders = PendingRiders::default();
        pending_riders.0.push_back(PendingRider {
            position: cell,
            destination,
            request_time_ms: 1000,
        });
        world.insert_resource(pending_riders);

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
            .schedule_at_secs(1, EventKind::RequestInbound, None);

        let mut schedule = simulation_schedule();
        let steps = run_until_empty(&mut world, &mut schedule, 1000);
        assert!(steps < 1000, "runner did not converge");

        let trip_entity = world
            .query::<bevy_ecs::prelude::Entity>()
            .iter(&world)
            .find(|entity| world.entity(*entity).contains::<Trip>())
            .expect("trip entity");
        let trip = world.entity(trip_entity).get::<Trip>().expect("trip");

        let driver = world
            .get_entity(driver_entity)
            .and_then(|e| e.get::<Driver>())
            .expect("driver");

        assert_eq!(trip.state, TripState::Completed);
        assert_eq!(trip.driver, driver_entity);
        assert_eq!(trip.pickup, cell);
        assert_eq!(trip.dropoff, destination, "dropoff should match the requested destination");
        assert_ne!(trip.dropoff, trip.pickup, "dropoff should differ from pickup");
        assert_eq!(driver.state, DriverState::Idle);
        assert_eq!(driver.matched_rider, None);

        let telemetry = world.resource::<SimTelemetry>();
        assert_eq!(telemetry.completed_trips.len(), 1);
        let record = &telemetry.completed_trips[0];
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
        world.insert_resource(SimSnapshotConfig::default());
        world.insert_resource(SimSnapshots::default());
        world.insert_resource(SpeedModel::with_range(Some(2), 40.0, 40.0));

        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
        // Pick a neighbor cell as destination
        let destination = cell
            .grid_disk::<Vec<_>>(1)
            .into_iter()
            .find(|c| *c != cell)
            .unwrap_or(cell);

        // Add two pending riders
        let mut pending_riders = PendingRiders::default();
        pending_riders.0.push_back(PendingRider {
            position: cell,
            destination,
            request_time_ms: 1000,
        });
        pending_riders.0.push_back(PendingRider {
            position: cell,
            destination,
            request_time_ms: 2000,
        });
        world.insert_resource(pending_riders);

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
            .schedule_at_secs(1, EventKind::RequestInbound, None);
        world
            .resource_mut::<SimulationClock>()
            .schedule_at_secs(2, EventKind::RequestInbound, None);

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

        let driver1_state = world.entity(driver1).get::<Driver>().expect("driver1").state;
        let driver2_state = world.entity(driver2).get::<Driver>().expect("driver2").state;
        assert_eq!(driver1_state, DriverState::Idle);
        assert_eq!(driver2_state, DriverState::Idle);

        let telemetry = world.resource::<SimTelemetry>();
        assert_eq!(telemetry.completed_trips.len(), 2);
        let drivers: Vec<_> = telemetry
            .completed_trips
            .iter()
            .map(|r| r.driver_entity)
            .collect();
        assert!(drivers.contains(&driver1));
        assert!(drivers.contains(&driver2));
        for record in &telemetry.completed_trips {
            assert!(record.completed_at >= ONE_SEC_MS);
        }
    }
}
