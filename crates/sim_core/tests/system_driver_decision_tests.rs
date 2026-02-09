mod support;

use bevy_ecs::prelude::{Entity, Schedule, World};
use bevy_ecs::schedule::apply_deferred;
use sim_core::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use sim_core::ecs::{
    Driver, DriverEarnings, DriverFatigue, EnRoute, Evaluating, GeoPosition, Idle, Position, Rider,
    Trip, TripEnRoute, Waiting,
};
use sim_core::scenario::DriverDecisionConfig;
use sim_core::systems::driver_decision::driver_decision_system;

fn setup_world(
    config: DriverDecisionConfig,
    accepted_fare: f64,
) -> (World, Entity, Entity, h3o::CellIndex, h3o::CellIndex) {
    let mut world = World::new();
    world.insert_resource(SimulationClock::default());
    world.insert_resource(config);
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
                accepted_fare: Some(accepted_fare),
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
            Evaluating,
            Position(cell),
            GeoPosition(cell.into()),
            DriverEarnings {
                daily_earnings: 0.0,
                daily_earnings_target: 200.0,
                session_start_time_ms: 0,
                session_end_time_ms: None,
            },
            DriverFatigue {
                fatigue_threshold_ms: 8 * 3600 * 1000,
            },
        ))
        .id();

    (world, rider_entity, driver_entity, cell, destination)
}

fn run_driver_decision(world: &mut World, driver_entity: Entity) {
    world.resource_mut::<SimulationClock>().schedule_at_secs(
        1,
        EventKind::DriverDecision,
        Some(EventSubject::Driver(driver_entity)),
    );

    let event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("driver decision event");
    world.insert_resource(CurrentEvent(event));

    let mut schedule = Schedule::default();
    schedule.add_systems((driver_decision_system, apply_deferred));
    schedule.run(world);
}

#[test]
fn evaluating_driver_moves_to_en_route() {
    let (mut world, rider_entity, driver_entity, cell, destination) = setup_world(
        DriverDecisionConfig {
            seed: 42,
            base_acceptance_score: 10.0,
            ..Default::default()
        },
        15.0,
    );

    run_driver_decision(&mut world, driver_entity);

    assert!(world.entity(driver_entity).contains::<EnRoute>());

    let next_event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("move step event");
    assert_eq!(next_event.kind, EventKind::MoveStep);
    assert_eq!(next_event.timestamp, 2000);

    let trip_entity = match next_event.subject {
        Some(EventSubject::Trip(trip_entity)) => trip_entity,
        other => panic!("expected trip subject, got {other:?}"),
    };
    let trip = world.entity(trip_entity).get::<Trip>().expect("trip");
    assert!(world.entity(trip_entity).contains::<TripEnRoute>());
    assert_eq!(trip.driver, driver_entity);
    assert_eq!(trip.rider, rider_entity);
    assert_eq!(trip.pickup, cell);
    assert_eq!(trip.dropoff, destination);
}

#[test]
fn driver_rejects_with_very_negative_score() {
    let (mut world, rider_entity, driver_entity, _, _) = setup_world(
        DriverDecisionConfig {
            seed: 42,
            base_acceptance_score: -100.0,
            ..Default::default()
        },
        5.0,
    );

    run_driver_decision(&mut world, driver_entity);

    let driver = world.query::<&Driver>().single(&world);
    assert!(world.entity(driver_entity).contains::<Idle>());
    assert_eq!(driver.matched_rider, None);

    let next_event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("match rejected event");
    assert_eq!(next_event.kind, EventKind::MatchRejected);
    assert_eq!(next_event.timestamp, 1000);
    assert_eq!(next_event.subject, Some(EventSubject::Rider(rider_entity)));
}

#[test]
fn driver_decision_is_reproducible_with_same_seed() {
    let (mut world1, rider_entity1, driver_entity1, _, _) = setup_world(
        DriverDecisionConfig {
            seed: 12345,
            base_acceptance_score: 0.0,
            ..Default::default()
        },
        10.0,
    );
    let (mut world2, rider_entity2, driver_entity2, _, _) = setup_world(
        DriverDecisionConfig {
            seed: 12345,
            base_acceptance_score: 0.0,
            ..Default::default()
        },
        10.0,
    );

    assert_eq!(driver_entity1.index(), driver_entity2.index());
    assert_eq!(rider_entity1.index(), rider_entity2.index());

    run_driver_decision(&mut world1, driver_entity1);
    run_driver_decision(&mut world2, driver_entity2);

    assert_eq!(
        world1.entity(driver_entity1).contains::<EnRoute>(),
        world2.entity(driver_entity2).contains::<EnRoute>()
    );
}
