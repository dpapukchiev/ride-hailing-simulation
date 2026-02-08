mod support;

use bevy_ecs::prelude::{Entity, World};
use sim_core::clock::{SimulationClock, ONE_SEC_MS};
use sim_core::distributions::UniformInterArrival;
use sim_core::ecs::{Driver, Idle, OffDuty, Trip, TripCompleted};
use sim_core::pricing::PricingConfig;
use sim_core::routing::{H3GridRouteProvider, RouteProviderResource};
use sim_core::runner::initialize_simulation;
use sim_core::scenario::{
    create_simple_matching, MatchRadius, RiderCancelConfig, RiderQuoteConfig, SimulationEndTimeMs,
};
use sim_core::spatial::SpatialIndex;
use sim_core::spawner::{DriverSpawner, DriverSpawnerConfig, RiderSpawner, RiderSpawnerConfig};
use sim_core::speed::SpeedModel;
use sim_core::telemetry::{SimSnapshotConfig, SimSnapshots, SimTelemetry};
use sim_core::traffic::{CongestionZones, DynamicCongestionConfig, TrafficProfile};
use support::schedule::ScheduleRunner;

fn setup_end_to_end_world(speed_seed: u64, rider_count: usize, rider_end_time_ms: u64) -> World {
    let mut world = World::new();
    world.insert_resource(SimulationClock::default());
    world.insert_resource(SimTelemetry::default());
    world.insert_resource(SimSnapshotConfig::default());
    world.insert_resource(SimSnapshots::default());
    world.insert_resource(SpeedModel::with_range(Some(speed_seed), 40.0, 40.0));
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
    let coord: h3o::LatLng = cell.into();
    let lat = coord.lat();
    let lng = coord.lng();
    let spread = if rider_count == 1 { 0.01 } else { 0.0001 };

    let rider_spawner_config = RiderSpawnerConfig {
        inter_arrival_dist: Box::new(UniformInterArrival::new(1000.0)),
        lat_min: lat - spread,
        lat_max: lat + spread,
        lng_min: lng - spread,
        lng_max: lng + spread,
        min_trip_cells: 2,
        max_trip_cells: 5,
        start_time_ms: Some(1000),
        end_time_ms: Some(rider_end_time_ms),
        max_count: Some(rider_count),
        initial_count: 0,
        seed: 42,
    };
    world.insert_resource(RiderSpawner::new(rider_spawner_config));

    let driver_spawner_config = DriverSpawnerConfig {
        inter_arrival_dist: Box::new(UniformInterArrival::new(1.0)),
        lat_min: lat - 0.0001,
        lat_max: lat + 0.0001,
        lng_min: lng - 0.0001,
        lng_max: lng + 0.0001,
        start_time_ms: Some(0),
        end_time_ms: Some(100),
        max_count: Some(rider_count),
        initial_count: 0,
        seed: 42,
    };
    world.insert_resource(DriverSpawner::new(driver_spawner_config));
    world.insert_resource(SimulationEndTimeMs(3_600_000));
    initialize_simulation(&mut world);
    world
}

#[test]
fn simulates_one_ride_end_to_end() {
    let mut world = setup_end_to_end_world(1, 1, 2000);
    let mut runner = ScheduleRunner::new();
    let steps = runner.run_until_empty(&mut world, 1000);
    assert!(steps < 1000, "runner did not converge");

    let trip_entity = world
        .query::<Entity>()
        .iter(&world)
        .find(|entity| world.entity(*entity).contains::<Trip>())
        .expect("trip entity");

    let drivers: Vec<_> = world.query::<(Entity, &Driver)>().iter(&world).collect();
    assert_eq!(drivers.len(), 1);
    let (driver_entity, driver) = drivers[0];

    let trip = world.entity(trip_entity).get::<Trip>().expect("trip");
    assert!(world.entity(trip_entity).contains::<TripCompleted>());
    assert_eq!(trip.driver, driver_entity);
    assert_ne!(
        trip.dropoff, trip.pickup,
        "dropoff should differ from pickup"
    );
    assert!(
        world.entity(driver_entity).contains::<Idle>()
            || world.entity(driver_entity).contains::<OffDuty>()
    );
    assert_eq!(driver.matched_rider, None);

    let telemetry = world.resource::<SimTelemetry>();
    assert_eq!(telemetry.completed_trips.len(), 1);
    let record = &telemetry.completed_trips[0];
    assert_eq!(record.driver_entity, driver_entity);
    assert_eq!(record.trip_entity, trip_entity);
    assert!(record.completed_at >= ONE_SEC_MS);
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
    let mut world = setup_end_to_end_world(2, 2, 3000);
    let mut runner = ScheduleRunner::new();
    let steps = runner.run_until_empty(&mut world, 1000);
    assert!(steps < 1000, "runner did not converge");

    let trips: Vec<_> = world.query::<(Entity, &Trip)>().iter(&world).collect();
    assert_eq!(trips.len(), 2, "expected two completed trips");
    for (trip_entity, _trip) in &trips {
        assert!(world.entity(*trip_entity).contains::<TripCompleted>());
    }

    let driver_entities: Vec<_> = world
        .query::<(Entity, &Driver)>()
        .iter(&world)
        .map(|(e, _)| e)
        .collect();
    assert_eq!(driver_entities.len(), 2);
    for driver_entity in driver_entities {
        assert!(
            world.entity(driver_entity).contains::<Idle>()
                || world.entity(driver_entity).contains::<OffDuty>()
        );
    }

    let telemetry = world.resource::<SimTelemetry>();
    assert_eq!(telemetry.completed_trips.len(), 2);
    for record in &telemetry.completed_trips {
        assert!(record.completed_at >= ONE_SEC_MS);
    }
}
