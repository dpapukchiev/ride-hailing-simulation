//! ECS Systems: event-driven logic that reacts to simulation events.
//!
//! Systems are functions that query and mutate the ECS world based on the current
//! event. Each system handles one aspect of the simulation lifecycle:
//!
//! - **Spawners**: Create riders and drivers dynamically
//! - **Matching**: Pair riders with available drivers
//! - **Movement**: Move drivers toward pickup/dropoff locations
//! - **State Transitions**: Update entity states (browsing → waiting → matched, etc.)
//! - **Telemetry**: Capture snapshots for visualization/export
//!
//! Systems react to the `CurrentEvent` resource, which is inserted by the runner
//! before each schedule execution.

pub mod batch_matching;
pub mod driver_decision;
pub mod driver_offduty;
pub mod match_accepted;
pub mod match_rejected;
pub mod matching;
pub mod movement;
pub mod pickup_eta_updated;
pub mod quote_accepted;
pub mod quote_decision;
pub mod quote_rejected;
pub mod rider_cancel;
pub mod show_quote;
pub mod spatial_index;
pub mod spawner;
pub mod telemetry_snapshot;
pub mod trip_completed;
pub mod trip_started;

#[cfg(test)]
mod end_to_end_tests {
    use bevy_ecs::prelude::World;

    use crate::clock::{SimulationClock, ONE_SEC_MS};
    use crate::distributions::UniformInterArrival;
    use crate::ecs::{Driver, Idle, OffDuty, Trip, TripCompleted};
    use crate::pricing::PricingConfig;
    use crate::routing::{H3GridRouteProvider, RouteProviderResource};
    use crate::runner::{initialize_simulation, run_until_empty, simulation_schedule};
    use crate::scenario::{
        create_simple_matching, MatchRadius, RiderCancelConfig, RiderQuoteConfig,
        SimulationEndTimeMs,
    };
    use crate::spatial::SpatialIndex;
    use crate::spawner::{DriverSpawner, DriverSpawnerConfig, RiderSpawner, RiderSpawnerConfig};
    use crate::speed::SpeedModel;
    use crate::telemetry::{SimSnapshotConfig, SimSnapshots, SimTelemetry};
    use crate::traffic::{CongestionZones, DynamicCongestionConfig, TrafficProfile};

    #[test]
    fn simulates_one_ride_end_to_end() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(SimTelemetry::default());
        world.insert_resource(SimSnapshotConfig::default());
        world.insert_resource(SimSnapshots::default());
        world.insert_resource(SpeedModel::with_range(Some(1), 40.0, 40.0));
        world.insert_resource(create_simple_matching());
        world.insert_resource(MatchRadius(0));
        world.insert_resource(RiderCancelConfig::default());
        world.insert_resource(RiderQuoteConfig {
            accept_probability: 1.0,
            ..Default::default()
        });
        world.insert_resource(PricingConfig::default());
        world.insert_resource(SpatialIndex::new());
        world.insert_resource(RouteProviderResource(Box::new(H3GridRouteProvider)));
        world.insert_resource(TrafficProfile::none());
        world.insert_resource(CongestionZones::default());
        world.insert_resource(DynamicCongestionConfig::default());

        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");

        // Configure spawners: spawn one rider at 1 second, one driver at 0
        let coord: h3o::LatLng = cell.into();
        let lat = coord.lat();
        let lng = coord.lng();
        let rider_spawner_config = RiderSpawnerConfig {
            inter_arrival_dist: Box::new(UniformInterArrival::new(1000.0)), // 1 second
            lat_min: lat - 0.01, // Wider bounds to allow destination selection
            lat_max: lat + 0.01,
            lng_min: lng - 0.01,
            lng_max: lng + 0.01,
            min_trip_cells: 2,
            max_trip_cells: 5,
            start_time_ms: Some(1000),
            end_time_ms: Some(2000),
            max_count: Some(1),
            initial_count: 0,
            seed: 42, // Test seed
        };
        world.insert_resource(RiderSpawner::new(rider_spawner_config));

        let driver_spawner_config = DriverSpawnerConfig {
            inter_arrival_dist: Box::new(UniformInterArrival::new(1.0)),
            lat_min: lat - 0.0001, // Tighter bounds to ensure same cell
            lat_max: lat + 0.0001,
            lng_min: lng - 0.0001,
            lng_max: lng + 0.0001,
            start_time_ms: Some(0),
            end_time_ms: Some(100),
            max_count: Some(1),
            initial_count: 0,
            seed: 42, // Test seed
        };
        world.insert_resource(DriverSpawner::new(driver_spawner_config));
        // OffDuty checks run indefinitely; set an end time so the event queue drains.
        world.insert_resource(SimulationEndTimeMs(3_600_000)); // 1 hour

        initialize_simulation(&mut world);

        let mut schedule = simulation_schedule();
        let steps = run_until_empty(&mut world, &mut schedule, 1000);
        assert!(steps < 1000, "runner did not converge");

        let trip_entity = world
            .query::<bevy_ecs::prelude::Entity>()
            .iter(&world)
            .find(|entity| world.entity(*entity).contains::<Trip>())
            .expect("trip entity");

        let drivers: Vec<_> = world
            .query::<(bevy_ecs::prelude::Entity, &Driver)>()
            .iter(&world)
            .collect();
        assert_eq!(drivers.len(), 1);
        let (driver_entity, driver) = drivers[0];

        let trip = world.entity(trip_entity).get::<Trip>().expect("trip");
        assert!(
            world.entity(trip_entity).contains::<TripCompleted>(),
            "trip should be in Completed state"
        );
        assert_eq!(trip.driver, driver_entity);
        // Note: pickup cell may differ from expected due to spawner randomness within bounds
        assert_ne!(
            trip.dropoff, trip.pickup,
            "dropoff should differ from pickup"
        );
        // Driver should be Idle or OffDuty (if earnings/fatigue thresholds were met)
        assert!(
            world.entity(driver_entity).contains::<Idle>()
                || world.entity(driver_entity).contains::<OffDuty>(),
            "driver should be Idle or OffDuty after trip completion"
        );
        assert_eq!(driver.matched_rider, None);

        let telemetry = world.resource::<SimTelemetry>();
        assert_eq!(telemetry.completed_trips.len(), 1);
        let record = &telemetry.completed_trips[0];
        assert_eq!(record.driver_entity, driver_entity);
        assert_eq!(record.trip_entity, trip_entity);
        assert!(
            record.completed_at >= ONE_SEC_MS,
            "completed_at should be in ms (>= 1s)"
        );
        assert!(record.requested_at <= record.matched_at);
        assert!(record.matched_at <= record.pickup_at);
        assert!(record.pickup_at <= record.completed_at);
        assert_eq!(
            record.time_to_match(),
            record.matched_at - record.requested_at
        );
        assert_eq!(
            record.time_to_pickup(),
            record.pickup_at - record.matched_at
        );
        assert_eq!(
            record.trip_duration(),
            record.completed_at - record.pickup_at
        );
    }

    #[test]
    fn simulates_two_concurrent_rides_end_to_end() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(SimTelemetry::default());
        world.insert_resource(SimSnapshotConfig::default());
        world.insert_resource(SimSnapshots::default());
        world.insert_resource(SpeedModel::with_range(Some(2), 40.0, 40.0));
        world.insert_resource(create_simple_matching());
        world.insert_resource(MatchRadius(0));
        world.insert_resource(RiderCancelConfig::default());
        world.insert_resource(RiderQuoteConfig {
            accept_probability: 1.0,
            ..Default::default()
        });
        world.insert_resource(PricingConfig::default());
        world.insert_resource(SpatialIndex::new());
        world.insert_resource(RouteProviderResource(Box::new(H3GridRouteProvider)));
        world.insert_resource(TrafficProfile::none());
        world.insert_resource(CongestionZones::default());
        world.insert_resource(DynamicCongestionConfig::default());

        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");

        // Configure spawners: spawn two riders at 1s and 2s, two drivers at 0
        let coord: h3o::LatLng = cell.into();
        let lat = coord.lat();
        let lng = coord.lng();
        let rider_spawner_config = RiderSpawnerConfig {
            inter_arrival_dist: Box::new(UniformInterArrival::new(1000.0)), // 1 second between spawns
            lat_min: lat - 0.0001, // Tighter bounds to ensure same cell
            lat_max: lat + 0.0001,
            lng_min: lng - 0.0001,
            lng_max: lng + 0.0001,
            min_trip_cells: 2,
            max_trip_cells: 5,
            start_time_ms: Some(1000),
            end_time_ms: Some(3000),
            max_count: Some(2),
            initial_count: 0,
            seed: 42, // Test seed
        };
        world.insert_resource(RiderSpawner::new(rider_spawner_config));

        let driver_spawner_config = DriverSpawnerConfig {
            inter_arrival_dist: Box::new(UniformInterArrival::new(1.0)),
            lat_min: lat - 0.0001, // Tighter bounds to ensure same cell
            lat_max: lat + 0.0001,
            lng_min: lng - 0.0001,
            lng_max: lng + 0.0001,
            start_time_ms: Some(0),
            end_time_ms: Some(100),
            max_count: Some(2),
            initial_count: 0,
            seed: 42, // Test seed
        };
        world.insert_resource(DriverSpawner::new(driver_spawner_config));
        // OffDuty checks run indefinitely; set an end time so the event queue drains.
        world.insert_resource(SimulationEndTimeMs(3_600_000)); // 1 hour

        initialize_simulation(&mut world);

        let mut schedule = simulation_schedule();
        let steps = run_until_empty(&mut world, &mut schedule, 1000);
        assert!(steps < 1000, "runner did not converge");

        let trips: Vec<_> = world
            .query::<(bevy_ecs::prelude::Entity, &Trip)>()
            .iter(&world)
            .collect();
        assert_eq!(trips.len(), 2, "expected two completed trips");
        for (trip_entity, _trip) in &trips {
            assert!(
                world.entity(*trip_entity).contains::<TripCompleted>(),
                "trip should be in Completed state"
            );
        }

        let driver_entities: Vec<_> = world
            .query::<(bevy_ecs::prelude::Entity, &Driver)>()
            .iter(&world)
            .map(|(e, _)| e)
            .collect();
        assert_eq!(driver_entities.len(), 2);
        for driver_entity in driver_entities {
            // Drivers should be Idle or OffDuty (if earnings/fatigue thresholds were met)
            assert!(
                world.entity(driver_entity).contains::<Idle>()
                    || world.entity(driver_entity).contains::<OffDuty>(),
                "driver should be Idle or OffDuty after trip completion"
            );
        }

        let telemetry = world.resource::<SimTelemetry>();
        assert_eq!(telemetry.completed_trips.len(), 2);
        for record in &telemetry.completed_trips {
            assert!(record.completed_at >= ONE_SEC_MS);
        }
    }
}
