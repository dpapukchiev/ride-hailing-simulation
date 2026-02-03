//! Simulation time: millisecond-scale timeline with a real-world epoch.
//!
//! All timestamps and `clock.now()` are in **simulation milliseconds**. Time 0 is
//! mapped to a real-world datetime via `epoch_ms`. The timeline advances by
//! popping the next scheduled event (same-ms events are ordered by `EventKind`).

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use bevy_ecs::prelude::{Entity, Resource};

/// One second in simulation milliseconds.
pub const ONE_SEC_MS: u64 = 1000;
/// One minute in simulation milliseconds.
pub const ONE_MIN_MS: u64 = 60 * ONE_SEC_MS;
/// One hour in simulation milliseconds.
pub const ONE_HOUR_MS: u64 = 60 * ONE_MIN_MS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventKind {
    SimulationStarted,
    SpawnRider,
    SpawnDriver,
    QuoteAccepted,
    TryMatch,
    MatchAccepted,
    DriverDecision,
    MoveStep,
    PickupEtaUpdated,
    TripStarted,
    TripCompleted,
    RiderCancel,
    CheckDriverOffDuty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventSubject {
    Rider(Entity),
    Driver(Entity),
    Trip(Entity),
}

/// Simulation event. `timestamp` is in **milliseconds** (simulation time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub timestamp: u64,
    pub kind: EventKind,
    pub subject: Option<EventSubject>,
}

impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap by timestamp; same timestamp ordered by kind for determinism.
        other
            .timestamp
            .cmp(&self.timestamp)
            .then_with(|| self.kind.cmp(&other.kind))
    }
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Copy, Resource)]
pub struct CurrentEvent(pub Event);

/// Simulation clock: time in **milliseconds**, advances to the next scheduled event.
/// Time 0 maps to a real-world datetime via `epoch_ms` (e.g. Unix epoch offset).
#[derive(Debug, Clone, Resource)]
pub struct SimulationClock {
    /// Current simulation time in ms (updated when an event is popped).
    now: u64,
    /// Real-world ms corresponding to simulation time 0 (e.g. Unix epoch or a fixed datetime).
    epoch_ms: i64,
    events: BinaryHeap<Event>,
}

impl Default for SimulationClock {
    fn default() -> Self {
        Self {
            now: 0,
            epoch_ms: 0,
            events: BinaryHeap::new(),
        }
    }
}

impl SimulationClock {
    /// Clock with time 0 mapped to the given real-world ms (e.g. from a datetime).
    pub fn with_epoch(epoch_ms: i64) -> Self {
        Self {
            now: 0,
            epoch_ms,
            events: BinaryHeap::new(),
        }
    }

    /// Current simulation time in milliseconds.
    pub fn now(&self) -> u64 {
        self.now
    }

    /// Current simulation time in seconds (now / 1000).
    pub fn now_secs(&self) -> u64 {
        self.now / ONE_SEC_MS
    }

    /// Current simulation time in minutes (now / 60_000).
    pub fn now_mins(&self) -> u64 {
        self.now / ONE_MIN_MS
    }

    /// Real-world ms that corresponds to simulation time 0.
    pub fn epoch_ms(&self) -> i64 {
        self.epoch_ms
    }

    /// Update the real-world epoch (ms) that maps to simulation time 0.
    pub fn set_epoch_ms(&mut self, epoch_ms: i64) {
        self.epoch_ms = epoch_ms;
    }

    /// Convert simulation ms to real-world ms (epoch_ms + sim_ms).
    pub fn sim_to_real_ms(&self, sim_ms: u64) -> i64 {
        self.epoch_ms.saturating_add(sim_ms as i64)
    }

    /// Convert real-world ms to simulation ms. Returns `None` if real_ms is before the epoch.
    pub fn real_to_sim_ms(&self, real_ms: i64) -> Option<u64> {
        let delta = real_ms.saturating_sub(self.epoch_ms);
        if delta < 0 {
            return None;
        }
        Some(delta as u64)
    }

    /// Schedule an event at a specific simulation timestamp (ms).
    pub fn schedule_at(
        &mut self,
        at_ms: u64,
        kind: EventKind,
        subject: Option<EventSubject>,
    ) {
        self.schedule(Event {
            timestamp: at_ms,
            kind,
            subject,
        });
    }

    /// Schedule an event at a simulation time in **seconds** (at_secs × 1000 ms).
    pub fn schedule_at_secs(
        &mut self,
        at_secs: u64,
        kind: EventKind,
        subject: Option<EventSubject>,
    ) {
        self.schedule_at(at_secs.saturating_mul(ONE_SEC_MS), kind, subject);
    }

    /// Schedule an event at a simulation time in **minutes** (at_mins × 60_000 ms).
    pub fn schedule_at_mins(
        &mut self,
        at_mins: u64,
        kind: EventKind,
        subject: Option<EventSubject>,
    ) {
        self.schedule_at(at_mins.saturating_mul(ONE_MIN_MS), kind, subject);
    }

    /// Schedule an event at `now + delta_ms` (relative, in ms).
    pub fn schedule_in(
        &mut self,
        delta_ms: u64,
        kind: EventKind,
        subject: Option<EventSubject>,
    ) {
        self.schedule_at(self.now.saturating_add(delta_ms), kind, subject);
    }

    /// Schedule an event in **delta_secs** seconds from now.
    pub fn schedule_in_secs(
        &mut self,
        delta_secs: u64,
        kind: EventKind,
        subject: Option<EventSubject>,
    ) {
        self.schedule_in(delta_secs.saturating_mul(ONE_SEC_MS), kind, subject);
    }

    /// Schedule an event in **delta_mins** minutes from now.
    pub fn schedule_in_mins(
        &mut self,
        delta_mins: u64,
        kind: EventKind,
        subject: Option<EventSubject>,
    ) {
        self.schedule_in(delta_mins.saturating_mul(ONE_MIN_MS), kind, subject);
    }

    /// Schedule a full event (for flexibility; timestamp must be in ms, >= now).
    pub fn schedule(&mut self, event: Event) {
        debug_assert!(
            event.timestamp >= self.now,
            "event timestamp must be >= current time"
        );
        self.events.push(event);
    }

    /// Pop the next event (earliest timestamp; same-ms order by kind). Advances `now` to that timestamp.
    pub fn pop_next(&mut self) -> Option<Event> {
        let event = self.events.pop()?;
        self.now = event.timestamp;
        Some(event)
    }

    /// Timestamp of the next scheduled event without popping it.
    pub fn next_event_time(&self) -> Option<u64> {
        self.events.peek().map(|event| event.timestamp)
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Number of events still in the queue (for tests and scenario validation).
    pub fn pending_event_count(&self) -> usize {
        self.events.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        // Same timestamp (20): QuoteAccepted < SpawnRider (enum order)
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
        let mut clock = SimulationClock::with_epoch(1_700_000_000_000); // example epoch
        clock.schedule_in_secs(1, EventKind::SpawnRider, None);
        let e = clock.pop_next().expect("event");
        assert_eq!(e.timestamp, ONE_SEC_MS);
        assert_eq!(clock.now(), ONE_SEC_MS);
        assert_eq!(clock.sim_to_real_ms(1000), 1_700_000_001_000);
        assert_eq!(clock.real_to_sim_ms(1_700_000_001_000), Some(1000));
        assert_eq!(clock.real_to_sim_ms(1_699_999_999_000), None);
    }
}
