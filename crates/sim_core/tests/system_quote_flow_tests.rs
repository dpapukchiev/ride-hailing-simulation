mod support;

use bevy_ecs::prelude::{Schedule, World};
use bevy_ecs::schedule::apply_deferred;
use sim_core::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock, ONE_SEC_MS};
use sim_core::ecs::{Browsing, GeoPosition, Position, Rider, RiderQuote, Waiting};
use sim_core::pricing::{PricingConfig, BASE_FARE};
use sim_core::scenario::{BatchMatchingConfig, RiderCancelConfig, RiderQuoteConfig};
use sim_core::systems::quote_accepted::quote_accepted_system;
use sim_core::systems::quote_decision::quote_decision_system;
use sim_core::systems::quote_rejected::quote_rejected_system;
use sim_core::systems::show_quote::show_quote_system;
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

fn schedule_current_event(world: &mut World, kind: EventKind, subject: EventSubject, at_secs: u64) {
    world
        .resource_mut::<SimulationClock>()
        .schedule_at_secs(at_secs, kind, Some(subject));
    let event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("scheduled event");
    world.insert_resource(CurrentEvent(event));
}

#[test]
fn quote_decision_with_accept_probability_one_schedules_accepted() {
    let mut world = World::new();
    world.insert_resource(SimulationClock::default());
    world.insert_resource(RiderQuoteConfig {
        max_quote_rejections: 3,
        re_quote_delay_secs: 10,
        accept_probability: 1.0,
        seed: 42,
        max_willingness_to_pay: 100.0,
        max_acceptable_eta_ms: 600_000,
    });
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
            Browsing,
            Position(cell),
            GeoPosition(cell.into()),
            RiderQuote {
                fare: 5.0,
                eta_ms: 60_000,
            },
        ))
        .id();

    schedule_current_event(
        &mut world,
        EventKind::QuoteDecision,
        EventSubject::Rider(rider_entity),
        1,
    );

    let mut schedule = Schedule::default();
    schedule.add_systems(quote_decision_system);
    schedule.run(&mut world);

    let next_event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("next event");
    assert_eq!(next_event.kind, EventKind::QuoteAccepted);
    assert_eq!(next_event.subject, Some(EventSubject::Rider(rider_entity)));
}

#[test]
fn quote_decision_with_accept_probability_zero_schedules_rejected() {
    let mut world = World::new();
    world.insert_resource(SimulationClock::default());
    world.insert_resource(RiderQuoteConfig {
        max_quote_rejections: 3,
        re_quote_delay_secs: 10,
        accept_probability: 0.0,
        seed: 42,
        max_willingness_to_pay: 100.0,
        max_acceptable_eta_ms: 600_000,
    });
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
            Browsing,
            Position(cell),
            GeoPosition(cell.into()),
            RiderQuote {
                fare: 5.0,
                eta_ms: 60_000,
            },
        ))
        .id();

    schedule_current_event(
        &mut world,
        EventKind::QuoteDecision,
        EventSubject::Rider(rider_entity),
        1,
    );

    let mut schedule = Schedule::default();
    schedule.add_systems(quote_decision_system);
    schedule.run(&mut world);

    let next_event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("next event");
    assert_eq!(next_event.kind, EventKind::QuoteRejected);
    assert_eq!(next_event.subject, Some(EventSubject::Rider(rider_entity)));
}

#[test]
fn quote_rejected_under_limit_reschedules_show_quote() {
    let mut world = World::new();
    world.insert_resource(SimulationClock::default());
    world.insert_resource(SimTelemetry::default());
    world.insert_resource(RiderQuoteConfig {
        max_quote_rejections: 3,
        re_quote_delay_secs: 10,
        accept_probability: 0.8,
        seed: 42,
        max_willingness_to_pay: 100.0,
        max_acceptable_eta_ms: 600_000,
    });
    let cell = seed_cell();
    let destination = neighbor_cell(cell);

    let rider_entity = world
        .spawn((
            Rider {
                matched_driver: None,
                assigned_trip: None,
                destination: Some(destination),
                requested_at: None,
                quote_rejections: 1,
                accepted_fare: None,
                last_rejection_reason: None,
            },
            Browsing,
            Position(cell),
            GeoPosition(cell.into()),
        ))
        .id();

    let now_ms = 5000;
    world.resource_mut::<SimulationClock>().schedule_at(
        now_ms,
        EventKind::QuoteRejected,
        Some(EventSubject::Rider(rider_entity)),
    );
    let event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("quote rejected event");
    world.insert_resource(CurrentEvent(event));

    let mut schedule = Schedule::default();
    schedule.add_systems((quote_rejected_system, apply_deferred));
    schedule.run(&mut world);

    let rider = world.get_entity(rider_entity).expect("rider still exists");
    let rider = rider.get::<Rider>().expect("Rider component");
    assert!(world.entity(rider_entity).contains::<Browsing>());
    assert_eq!(rider.quote_rejections, 2);

    let next_event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("show quote rescheduled");
    assert_eq!(next_event.kind, EventKind::ShowQuote);
    assert_eq!(next_event.subject, Some(EventSubject::Rider(rider_entity)));
    assert_eq!(next_event.timestamp, now_ms + 10 * 1000);
}

#[test]
fn quote_rejected_at_limit_despawns_and_increments_telemetry() {
    let mut world = World::new();
    world.insert_resource(SimulationClock::default());
    world.insert_resource(SimTelemetry::default());
    world.insert_resource(RiderQuoteConfig {
        max_quote_rejections: 2,
        re_quote_delay_secs: 10,
        accept_probability: 0.8,
        seed: 42,
        max_willingness_to_pay: 100.0,
        max_acceptable_eta_ms: 600_000,
    });
    let cell = seed_cell();
    let destination = neighbor_cell(cell);

    let rider_entity = world
        .spawn((
            Rider {
                matched_driver: None,
                assigned_trip: None,
                destination: Some(destination),
                requested_at: None,
                quote_rejections: 2,
                accepted_fare: None,
                last_rejection_reason: None,
            },
            Browsing,
            Position(cell),
            GeoPosition(cell.into()),
        ))
        .id();

    schedule_current_event(
        &mut world,
        EventKind::QuoteRejected,
        EventSubject::Rider(rider_entity),
        1,
    );

    let mut schedule = Schedule::default();
    schedule.add_systems((quote_rejected_system, apply_deferred));
    schedule.run(&mut world);

    assert!(
        world.get_entity(rider_entity).is_none(),
        "rider should be despawned on give-up"
    );
    let telemetry = world.resource::<SimTelemetry>();
    assert_eq!(telemetry.riders_abandoned_quote_total, 1);
}

#[test]
fn quote_accepted_transitions_rider_state() {
    let mut world = World::new();
    world.insert_resource(SimulationClock::default());
    world.insert_resource(RiderCancelConfig::default());
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
            Browsing,
            RiderQuote {
                fare: 12.5,
                eta_ms: 60_000,
            },
        ))
        .id();

    schedule_current_event(
        &mut world,
        EventKind::QuoteAccepted,
        EventSubject::Rider(rider_entity),
        1,
    );

    let mut schedule = Schedule::default();
    schedule.add_systems((quote_accepted_system, apply_deferred));
    schedule.run(&mut world);

    let rider = world.query::<&Rider>().single(&world);
    assert!(world.entity(rider_entity).contains::<Waiting>());
    assert_eq!(rider.accepted_fare, Some(12.5));

    let next_event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("try match event");
    assert_eq!(next_event.kind, EventKind::TryMatch);
    assert_eq!(next_event.timestamp, 2000);
    assert_eq!(next_event.subject, Some(EventSubject::Rider(rider_entity)));

    let cancel_event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("rider cancel event");
    assert_eq!(cancel_event.kind, EventKind::RiderCancel);
    let config = world.resource::<RiderCancelConfig>();
    let min_timestamp = 1000 + config.min_wait_secs * 1000;
    let max_timestamp = 1000 + config.max_wait_secs * 1000;
    assert!(cancel_event.timestamp >= min_timestamp && cancel_event.timestamp <= max_timestamp);
    assert_eq!(
        cancel_event.subject,
        Some(EventSubject::Rider(rider_entity))
    );
}

#[test]
fn show_quote_computes_quote_and_schedules_quote_decision() {
    let mut world = World::new();
    world.insert_resource(SimulationClock::default());
    world.insert_resource(PricingConfig::default());
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
            Browsing,
            Position(cell),
            GeoPosition(cell.into()),
        ))
        .id();

    schedule_current_event(
        &mut world,
        EventKind::ShowQuote,
        EventSubject::Rider(rider_entity),
        1,
    );

    let mut schedule = Schedule::default();
    schedule.add_systems(show_quote_system);
    schedule.run(&mut world);

    let rider_quote = world.get::<RiderQuote>(rider_entity).expect("RiderQuote");
    assert!(rider_quote.fare >= BASE_FARE);
    assert!(rider_quote.eta_ms >= ONE_SEC_MS);

    let next_event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("quote decision event");
    assert_eq!(next_event.kind, EventKind::QuoteDecision);
    assert_eq!(next_event.subject, Some(EventSubject::Rider(rider_entity)));
}

#[test]
fn quote_accepted_does_not_schedule_try_match_when_batch_matching_enabled() {
    let mut world = World::new();
    world.insert_resource(SimulationClock::default());
    world.insert_resource(RiderCancelConfig::default());
    world.insert_resource(BatchMatchingConfig {
        enabled: true,
        interval_secs: 5,
    });
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
            Browsing,
            RiderQuote {
                fare: 10.0,
                eta_ms: 60_000,
            },
        ))
        .id();

    schedule_current_event(
        &mut world,
        EventKind::QuoteAccepted,
        EventSubject::Rider(rider_entity),
        1,
    );

    let mut schedule = Schedule::default();
    schedule.add_systems((quote_accepted_system, apply_deferred));
    schedule.run(&mut world);

    let first = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("rider cancel event");
    assert_eq!(first.kind, EventKind::RiderCancel);
    assert!(world.resource::<SimulationClock>().is_empty());
}
