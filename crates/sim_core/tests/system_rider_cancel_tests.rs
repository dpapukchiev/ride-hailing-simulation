mod support;

use bevy_ecs::prelude::{Schedule, World};
use bevy_ecs::schedule::apply_deferred;
use sim_core::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use sim_core::ecs::{
    Driver, EnRoute, GeoPosition, Idle, Position, Rider, Trip, TripCancelled, TripEnRoute,
    TripFinancials, TripLiveData, TripTiming, Waiting,
};
use sim_core::systems::rider_cancel::rider_cancel_system;
use sim_core::telemetry::SimTelemetry;

fn seed_cell() -> h3o::CellIndex {
    h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell")
}

fn neighbor_cell(cell: h3o::CellIndex) -> h3o::CellIndex {
    cell.grid_disk::<Vec<_>>(1)
        .into_iter()
        .find(|candidate| *candidate != cell)
        .expect("neighbor cell")
}

#[test]
fn rider_cancel_resets_driver_and_trip() {
    let mut world = World::new();
    world.insert_resource(SimulationClock::default());
    world.insert_resource(SimTelemetry::default());
    let cell = seed_cell();
    let destination = neighbor_cell(cell);

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
            EnRoute,
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
            TripFinancials {
                agreed_fare: None,
                pickup_distance_km_at_accept: 0.0,
            },
            TripLiveData { pickup_eta_ms: 0 },
        ))
        .id();

    {
        let mut rider_entity_mut = world.entity_mut(rider_entity);
        let mut rider = rider_entity_mut.get_mut::<Rider>().expect("rider");
        rider.matched_driver = Some(driver_entity);
        rider.assigned_trip = Some(trip_entity);
    }
    {
        let mut driver_entity_mut = world.entity_mut(driver_entity);
        let mut driver = driver_entity_mut.get_mut::<Driver>().expect("driver");
        driver.assigned_trip = Some(trip_entity);
    }

    world.resource_mut::<SimulationClock>().schedule_at_secs(
        1,
        EventKind::RiderCancel,
        Some(EventSubject::Rider(rider_entity)),
    );
    let event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("rider cancel event");
    world.insert_resource(CurrentEvent(event));

    let mut schedule = Schedule::default();
    schedule.add_systems((rider_cancel_system, apply_deferred));
    schedule.run(&mut world);

    assert!(
        world.get_entity(rider_entity).is_none(),
        "rider should be despawned on cancel"
    );

    let driver = world.entity(driver_entity).get::<Driver>().expect("driver");
    assert!(world.entity(driver_entity).contains::<Idle>());
    assert_eq!(driver.matched_rider, None);

    let _trip = world.entity(trip_entity).get::<Trip>().expect("trip");
    assert!(world.entity(trip_entity).contains::<TripCancelled>());
}

#[test]
fn rider_cancel_without_match_despawns_rider() {
    let mut world = World::new();
    world.insert_resource(SimulationClock::default());
    world.insert_resource(SimTelemetry::default());
    let cell = seed_cell();
    let destination = neighbor_cell(cell);

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

    world.resource_mut::<SimulationClock>().schedule_at_secs(
        1,
        EventKind::RiderCancel,
        Some(EventSubject::Rider(rider_entity)),
    );
    let event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("rider cancel event");
    world.insert_resource(CurrentEvent(event));

    let mut schedule = Schedule::default();
    schedule.add_systems((rider_cancel_system, apply_deferred));
    schedule.run(&mut world);

    assert!(
        world.get_entity(rider_entity).is_none(),
        "rider should be despawned on cancel"
    );
}
