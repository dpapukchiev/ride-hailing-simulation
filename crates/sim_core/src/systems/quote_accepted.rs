use bevy_ecs::prelude::{Query, Res, ResMut};

use crate::clock::{CurrentEvent, Event, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Rider, RiderState};

pub fn quote_accepted_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    mut riders: Query<&mut Rider>,
) {
    if event.0.kind != EventKind::QuoteAccepted {
        return;
    }

    let Some(EventSubject::Rider(rider_entity)) = event.0.subject else {
        return;
    };
    let Ok(mut rider) = riders.get_mut(rider_entity) else {
        return;
    };
    if rider.state == RiderState::Browsing {
        rider.state = RiderState::Waiting;
    }

    let next_timestamp = clock.now() + 1;
    clock.schedule(Event {
        timestamp: next_timestamp,
        kind: EventKind::TryMatch,
        subject: Some(EventSubject::Rider(rider_entity)),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    use crate::clock::Event;

    #[test]
    fn quote_accepted_transitions_rider_state() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        let rider_entity = world
            .spawn(Rider {
                state: RiderState::Browsing,
                matched_driver: None,
                destination: None,
            })
            .id();

        world
            .resource_mut::<SimulationClock>()
            .schedule(Event {
                timestamp: 1,
                kind: EventKind::QuoteAccepted,
                subject: Some(EventSubject::Rider(rider_entity)),
            });

        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("quote accepted event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems(quote_accepted_system);
        schedule.run(&mut world);

        let rider = world.query::<&Rider>().single(&world);
        assert_eq!(rider.state, RiderState::Waiting);

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("try match event");
        assert_eq!(next_event.kind, EventKind::TryMatch);
        assert_eq!(next_event.timestamp, 2);
        assert_eq!(next_event.subject, Some(EventSubject::Rider(rider_entity)));
    }
}
