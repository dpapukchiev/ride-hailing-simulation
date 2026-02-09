mod support;

use bevy_ecs::prelude::Schedule;
use sim_core::clock::{CurrentEvent, EventKind, EventSubject};
use sim_core::ecs::{
    Driver, EnRoute, GeoPosition, Position, Rider, Trip, TripEnRoute, TripFinancials, TripLiveData,
    TripTiming, Waiting,
};
use sim_core::systems::movement::movement_system;

use support::world::TestWorldBuilder;

#[test]
fn movement_steps_toward_rider_and_schedules_trip_start() {
    let mut world = TestWorldBuilder::new().with_seed(1).build();

    let origin = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
    let neighbor = origin
        .grid_disk::<Vec<_>>(1)
        .into_iter()
        .find(|cell| *cell != origin)
        .expect("neighbor");
    let dropoff = origin
        .grid_disk::<Vec<_>>(2)
        .into_iter()
        .find(|cell| *cell != origin && *cell != neighbor)
        .expect("dropoff cell");

    let rider_entity = world
        .spawn((
            Rider {
                matched_driver: None,
                assigned_trip: None,
                destination: Some(dropoff),
                requested_at: None,
                quote_rejections: 0,
                accepted_fare: None,
                last_rejection_reason: None,
            },
            Waiting,
            Position(neighbor),
            GeoPosition(neighbor.into()),
        ))
        .id();
    let driver_entity = world
        .spawn((
            Driver {
                matched_rider: Some(rider_entity),
                assigned_trip: None,
            },
            EnRoute,
            Position(origin),
            GeoPosition(origin.into()),
        ))
        .id();
    let trip_entity = world
        .spawn((
            Trip {
                rider: rider_entity,
                driver: driver_entity,
                pickup: neighbor,
                dropoff,
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

    world
        .resource_mut::<sim_core::clock::SimulationClock>()
        .schedule_at_secs(
            1,
            EventKind::MoveStep,
            Some(EventSubject::Trip(trip_entity)),
        );
    let event = world
        .resource_mut::<sim_core::clock::SimulationClock>()
        .pop_next()
        .expect("move step event");
    world.insert_resource(CurrentEvent(event));

    let mut schedule = Schedule::default();
    schedule.add_systems(movement_system);
    schedule.run(&mut world);

    let driver_position = {
        let pos = world
            .query::<&Position>()
            .get(&world, driver_entity)
            .expect("pos");
        pos.0
    };
    assert_ne!(driver_position, origin);

    let eta_event = world
        .resource_mut::<sim_core::clock::SimulationClock>()
        .pop_next()
        .expect("pickup eta updated event");
    assert_eq!(eta_event.kind, EventKind::PickupEtaUpdated);
    assert_eq!(eta_event.timestamp, 1000);
    assert_eq!(eta_event.subject, Some(EventSubject::Trip(trip_entity)));

    let next_event = world
        .resource_mut::<sim_core::clock::SimulationClock>()
        .pop_next()
        .expect("next event");
    assert!(
        matches!(
            next_event.kind,
            EventKind::MoveStep | EventKind::TripStarted
        ),
        "unexpected next event: {:?}",
        next_event.kind
    );
    assert_eq!(next_event.subject, Some(EventSubject::Trip(trip_entity)));
}
