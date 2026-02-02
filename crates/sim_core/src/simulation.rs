use crate::agents::{Rider, RiderState};
use crate::clock::{Event, EventKind};

pub fn handle_event(event: Event, rider: &mut Rider) {
    match event.kind {
        EventKind::RequestInbound => {
            if rider.state == RiderState::Requesting {
                rider.state = RiderState::Waiting;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::SimulationClock;

    #[test]
    fn request_inbound_transitions_rider_state() {
        let mut rider = Rider::new(RiderState::Requesting);
        let mut clock = SimulationClock::default();
        clock.schedule(Event {
            timestamp: 1,
            kind: EventKind::RequestInbound,
        });

        let event = clock.pop_next().expect("event");
        handle_event(event, &mut rider);

        assert_eq!(rider.state, RiderState::Waiting);
    }
}
