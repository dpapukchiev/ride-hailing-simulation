mod support;

use bevy_ecs::prelude::{Entity, Schedule, World};
use bevy_ecs::schedule::apply_deferred;
use sim_core::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock, ONE_SEC_MS};
use sim_core::ecs::{Driver, Evaluating, GeoPosition, Idle, Position, Rider, Waiting};
use sim_core::matching::{MatchingAlgorithmResource, SimpleMatching};
use sim_core::scenario::BatchMatchingConfig;
use sim_core::systems::match_accepted::match_accepted_system;
use sim_core::systems::match_rejected::match_rejected_system;
use sim_core::systems::matching::matching_system;

fn seed_cell() -> h3o::CellIndex {
    h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell")
}

fn neighbor_cell(cell: h3o::CellIndex) -> h3o::CellIndex {
    cell.grid_disk::<Vec<_>>(1)
        .into_iter()
        .find(|candidate| *candidate != cell)
        .expect("neighbor cell")
}

fn setup_world_with_rejected_rider(batch_config: Option<BatchMatchingConfig>) -> (World, Entity) {
    let mut world = World::new();
    world.insert_resource(SimulationClock::default());
    if let Some(config) = batch_config {
        world.insert_resource(config);
    }

    let cell = seed_cell();
    let destination = neighbor_cell(cell);
    let fake_driver = world.spawn_empty().id();
    let rider_entity = world
        .spawn((
            Rider {
                matched_driver: Some(fake_driver),
                assigned_trip: None,
                destination: Some(destination),
                requested_at: None,
                quote_rejections: 0,
                accepted_fare: Some(10.0),
                last_rejection_reason: None,
            },
            Waiting,
            Position(cell),
            GeoPosition(cell.into()),
        ))
        .id();

    world.resource_mut::<SimulationClock>().schedule_at_secs(
        1,
        EventKind::MatchRejected,
        Some(EventSubject::Rider(rider_entity)),
    );
    let event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("match rejected event");
    world.insert_resource(CurrentEvent(event));

    (world, rider_entity)
}

#[test]
fn matches_waiting_rider_to_idle_driver() {
    let mut world = World::new();
    world.insert_resource(SimulationClock::default());
    world.insert_resource(MatchingAlgorithmResource::new(Box::new(SimpleMatching)));
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
                matched_rider: None,
                assigned_trip: None,
            },
            Idle,
            Position(cell),
            GeoPosition(cell.into()),
        ))
        .id();

    world.resource_mut::<SimulationClock>().schedule_at_secs(
        0,
        EventKind::TryMatch,
        Some(EventSubject::Rider(rider_entity)),
    );
    let event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("try match event");
    world.insert_resource(CurrentEvent(event));

    let mut schedule = Schedule::default();
    schedule.add_systems((matching_system, apply_deferred));
    schedule.run(&mut world);

    let (rider_waiting, matched_driver) = {
        let rider = world.query::<&Rider>().single(&world);
        (
            world.entity(rider_entity).contains::<Waiting>(),
            rider.matched_driver,
        )
    };
    let (driver_evaluating, matched_rider) = {
        let driver = world.query::<&Driver>().single(&world);
        (
            world.entity(driver_entity).contains::<Evaluating>(),
            driver.matched_rider,
        )
    };

    assert!(rider_waiting);
    assert!(driver_evaluating);
    assert_eq!(matched_driver, Some(driver_entity));
    assert_eq!(matched_rider, Some(rider_entity));

    let next_event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("match accepted event");
    assert_eq!(next_event.kind, EventKind::MatchAccepted);
    assert_eq!(next_event.timestamp, ONE_SEC_MS);
    assert_eq!(
        next_event.subject,
        Some(EventSubject::Driver(driver_entity))
    );
}

#[test]
fn match_accepted_schedules_driver_decision() {
    let mut world = World::new();
    world.insert_resource(SimulationClock::default());
    let driver_entity = world
        .spawn((
            Driver {
                matched_rider: None,
                assigned_trip: None,
            },
            Evaluating,
        ))
        .id();

    world.resource_mut::<SimulationClock>().schedule_at_secs(
        2,
        EventKind::MatchAccepted,
        Some(EventSubject::Driver(driver_entity)),
    );
    let event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("match accepted event");
    world.insert_resource(CurrentEvent(event));

    let mut schedule = Schedule::default();
    schedule.add_systems(match_accepted_system);
    schedule.run(&mut world);

    let next_event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("driver decision event");
    assert_eq!(next_event.kind, EventKind::DriverDecision);
    assert_eq!(next_event.timestamp, 3000);
    assert_eq!(
        next_event.subject,
        Some(EventSubject::Driver(driver_entity))
    );
}

#[test]
fn clears_matched_driver_and_schedules_retry() {
    let (mut world, rider_entity) = setup_world_with_rejected_rider(None);

    let mut schedule = Schedule::default();
    schedule.add_systems(match_rejected_system);
    schedule.run(&mut world);

    let rider = world.entity(rider_entity).get::<Rider>().expect("rider");
    assert_eq!(rider.matched_driver, None);

    let next_event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("retry event");
    assert_eq!(next_event.kind, EventKind::TryMatch);
    assert_eq!(next_event.subject, Some(EventSubject::Rider(rider_entity)));
    assert_eq!(next_event.timestamp, 1000 + 30 * 1000);
}

#[test]
fn no_retry_when_batch_matching_enabled() {
    let (mut world, rider_entity) = setup_world_with_rejected_rider(Some(BatchMatchingConfig {
        enabled: true,
        interval_secs: 5,
    }));

    let mut schedule = Schedule::default();
    schedule.add_systems(match_rejected_system);
    schedule.run(&mut world);

    let rider = world.entity(rider_entity).get::<Rider>().expect("rider");
    assert_eq!(rider.matched_driver, None);
    assert!(
        world.resource::<SimulationClock>().is_empty(),
        "no events should be scheduled when batch matching is enabled"
    );
}
