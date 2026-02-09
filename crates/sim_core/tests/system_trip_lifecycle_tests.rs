mod support;

use bevy_ecs::prelude::{Schedule, World};
use bevy_ecs::schedule::apply_deferred;
use sim_core::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use sim_core::ecs::{
    Driver, GeoPosition, InTransit, OnTrip, Position, Rider, Trip, TripCompleted, TripEnRoute,
    TripOnTrip, TripTiming, Waiting,
};
use sim_core::pricing::PricingConfig;
use sim_core::systems::trip_completed::trip_completed_system;
use sim_core::systems::trip_started::trip_started_system;
use sim_core::telemetry::SimTelemetry;

#[test]
fn trip_started_transitions_and_schedules_completion() {
    let mut world = World::new();
    world.insert_resource(SimulationClock::default());
    let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
    let destination = cell
        .grid_disk::<Vec<_>>(1)
        .into_iter()
        .find(|c| *c != cell)
        .expect("neighbor cell");

    let rider_entity = world
        .spawn((
            Rider {
                matched_driver: None,
                assigned_trip: None,
                destination: Some(destination),
                requested_at: None,
                quote_rejections: 0,
                accepted_fare: None,
                last_rejection_reason: None,
            },
            Waiting,
            Position(cell),
            GeoPosition(cell.into()),
        ))
        .id();
    let driver_entity = world
        .spawn((
            Driver {
                matched_rider: Some(rider_entity),
                assigned_trip: None,
            },
            sim_core::ecs::EnRoute,
            Position(cell),
            GeoPosition(cell.into()),
        ))
        .id();
    let trip_entity = world
        .spawn((
            Trip {
                rider: rider_entity,
                driver: driver_entity,
                pickup: cell,
                dropoff: destination,
            },
            TripEnRoute,
            TripTiming {
                requested_at: 0,
                matched_at: 0,
                pickup_at: None,
                dropoff_at: None,
                cancelled_at: None,
            },
            sim_core::ecs::TripFinancials {
                agreed_fare: None,
                pickup_distance_km_at_accept: 0.0,
            },
            sim_core::ecs::TripLiveData { pickup_eta_ms: 0 },
        ))
        .id();

    world
        .entity_mut(rider_entity)
        .get_mut::<Rider>()
        .expect("rider")
        .matched_driver = Some(driver_entity);

    world.resource_mut::<SimulationClock>().schedule_at_secs(
        3,
        EventKind::TripStarted,
        Some(EventSubject::Trip(trip_entity)),
    );

    let event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("trip started event");
    world.insert_resource(CurrentEvent(event));

    let mut schedule = Schedule::default();
    schedule.add_systems((trip_started_system, apply_deferred));
    schedule.run(&mut world);

    assert!(world.entity(rider_entity).contains::<InTransit>());
    assert!(world.entity(driver_entity).contains::<OnTrip>());
    assert!(world.entity(trip_entity).contains::<TripOnTrip>());

    let next_event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("move step toward dropoff");
    assert_eq!(next_event.kind, EventKind::MoveStep);
    assert_eq!(next_event.timestamp, 4000);
    assert_eq!(next_event.subject, Some(EventSubject::Trip(trip_entity)));
}

#[test]
fn trip_completed_transitions_driver_and_rider() {
    let mut world = World::new();
    world.insert_resource(SimulationClock::default());
    world.insert_resource(SimTelemetry::default());
    world.insert_resource(PricingConfig::default());

    let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
    let destination = cell
        .grid_disk::<Vec<_>>(1)
        .into_iter()
        .find(|c| *c != cell)
        .expect("neighbor cell");

    let rider_entity = world
        .spawn((
            Rider {
                matched_driver: None,
                assigned_trip: None,
                destination: Some(destination),
                requested_at: None,
                quote_rejections: 0,
                accepted_fare: None,
                last_rejection_reason: None,
            },
            InTransit,
        ))
        .id();
    let driver_entity = world
        .spawn((
            Driver {
                matched_rider: None,
                assigned_trip: None,
            },
            OnTrip,
            sim_core::ecs::DriverEarnings {
                daily_earnings: 0.0,
                daily_earnings_target: 100.0,
                session_start_time_ms: 0,
                session_end_time_ms: None,
            },
        ))
        .id();
    let trip_entity = world
        .spawn((
            Trip {
                rider: rider_entity,
                driver: driver_entity,
                pickup: cell,
                dropoff: destination,
            },
            TripOnTrip,
            TripTiming {
                requested_at: 0,
                matched_at: 1,
                pickup_at: Some(2),
                dropoff_at: None,
                cancelled_at: None,
            },
            sim_core::ecs::TripFinancials {
                agreed_fare: None,
                pickup_distance_km_at_accept: 0.0,
            },
            sim_core::ecs::TripLiveData { pickup_eta_ms: 0 },
        ))
        .id();

    world
        .entity_mut(rider_entity)
        .get_mut::<Rider>()
        .expect("rider")
        .matched_driver = Some(driver_entity);
    world
        .entity_mut(driver_entity)
        .get_mut::<Driver>()
        .expect("driver")
        .matched_rider = Some(rider_entity);

    world.resource_mut::<SimulationClock>().schedule_at_secs(
        2,
        EventKind::TripCompleted,
        Some(EventSubject::Trip(trip_entity)),
    );

    let event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("trip completed event");
    world.insert_resource(CurrentEvent(event));

    let mut schedule = Schedule::default();
    schedule.add_systems((trip_completed_system, apply_deferred));
    schedule.run(&mut world);

    let rider_exists = world.query::<&Rider>().iter(&world).next().is_some();
    let driver = world.entity(driver_entity).get::<Driver>().expect("driver");
    assert!(!rider_exists, "rider should be despawned on completion");
    assert!(world
        .entity(driver_entity)
        .contains::<sim_core::ecs::Idle>());
    assert_eq!(driver.matched_rider, None);
    assert!(world.entity(trip_entity).contains::<TripCompleted>());
}
