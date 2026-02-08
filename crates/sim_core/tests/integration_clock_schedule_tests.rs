mod support;

use sim_core::clock::{EventKind, SimulationClock, ONE_SEC_MS};

#[test]
fn clock_pops_events_in_time_order() {
    let mut clock = SimulationClock::default();
    clock.schedule_at(20, EventKind::SpawnRider, None);
    clock.schedule_at(5, EventKind::SpawnRider, None);
    clock.schedule_at(20, EventKind::QuoteAccepted, None);
    clock.schedule_at(10, EventKind::SpawnRider, None);

    let first = clock.pop_next().expect("first event");
    assert_eq!(first.timestamp, 5);
    assert_eq!(clock.now(), 5);

    let second = clock.pop_next().expect("second event");
    assert_eq!(second.timestamp, 10);
    assert_eq!(clock.now(), 10);

    let third = clock.pop_next().expect("third event");
    assert_eq!(third.timestamp, 20);
    assert_eq!(third.kind, EventKind::QuoteAccepted);
    let fourth = clock.pop_next().expect("fourth event");
    assert_eq!(fourth.timestamp, 20);
    assert_eq!(fourth.kind, EventKind::SpawnRider);

    assert!(clock.pop_next().is_none());
    assert!(clock.is_empty());
}

#[test]
fn schedule_in_and_conversion() {
    let mut clock = SimulationClock::with_epoch(1_700_000_000_000);
    clock.schedule_in_secs(1, EventKind::SpawnRider, None);
    let e = clock.pop_next().expect("event");
    assert_eq!(e.timestamp, ONE_SEC_MS);
    assert_eq!(clock.now(), ONE_SEC_MS);
    assert_eq!(clock.sim_to_real_ms(1000), 1_700_000_001_000);
    assert_eq!(clock.real_to_sim_ms(1_700_000_001_000), Some(1000));
    assert_eq!(clock.real_to_sim_ms(1_699_999_999_000), None);
}
