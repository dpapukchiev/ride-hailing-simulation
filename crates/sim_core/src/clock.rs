use std::cmp::Ordering;
use std::collections::BinaryHeap;

use bevy_ecs::prelude::Resource;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventKind {
    RequestInbound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub timestamp: u64,
    pub kind: EventKind,
}

impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering to make BinaryHeap a min-heap by timestamp.
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

#[derive(Debug, Default, Resource)]
pub struct SimulationClock {
    now: u64,
    events: BinaryHeap<Event>,
}

impl SimulationClock {
    pub fn now(&self) -> u64 {
        self.now
    }

    pub fn schedule(&mut self, event: Event) {
        debug_assert!(
            event.timestamp >= self.now,
            "event timestamp must be >= current time"
        );
        self.events.push(event);
    }

    pub fn pop_next(&mut self) -> Option<Event> {
        let event = self.events.pop()?;
        self.now = event.timestamp;
        Some(event)
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_pops_events_in_time_order() {
        let mut clock = SimulationClock::default();
        clock.schedule(Event {
            timestamp: 10,
            kind: EventKind::RequestInbound,
        });
        clock.schedule(Event {
            timestamp: 5,
            kind: EventKind::RequestInbound,
        });
        clock.schedule(Event {
            timestamp: 20,
            kind: EventKind::RequestInbound,
        });

        let first = clock.pop_next().expect("first event");
        assert_eq!(first.timestamp, 5);
        assert_eq!(clock.now(), 5);

        let second = clock.pop_next().expect("second event");
        assert_eq!(second.timestamp, 10);
        assert_eq!(clock.now(), 10);

        let third = clock.pop_next().expect("third event");
        assert_eq!(third.timestamp, 20);
        assert_eq!(clock.now(), 20);

        assert!(clock.pop_next().is_none());
        assert!(clock.is_empty());
    }
}
