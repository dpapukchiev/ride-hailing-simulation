use bevy_ecs::prelude::{Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Rider, RiderState};

pub fn request_inbound_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    mut riders: Query<&mut Rider>,
) {
    if event.0.kind != EventKind::RequestInbound {
        return;
    }

    let Some(EventSubject::Rider(rider_entity)) = event.0.subject else {
        return;
    };
    let Ok(mut rider) = riders.get_mut(rider_entity) else {
        return;
    };
    if rider.state == RiderState::Requesting {
        rider.state = RiderState::Browsing;
        rider.requested_at = Some(clock.now());
    }

    clock.schedule_in_secs(1, EventKind::QuoteAccepted, Some(EventSubject::Rider(rider_entity)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    use crate::clock::ONE_SEC_MS;

    #[test]
    fn ecs_system_transitions_rider_state() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        let rider_entity = world
            .spawn(Rider {
                state: RiderState::Requesting,
                matched_driver: None,
                destination: None,
                requested_at: None,
            })
            .id();

        world
            .resource_mut::<SimulationClock>()
            .schedule_at_secs(1, EventKind::RequestInbound, Some(EventSubject::Rider(rider_entity)));

        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("request inbound event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems(request_inbound_system);
        schedule.run(&mut world);

        let rider = world.query::<&Rider>().single(&world);
        assert_eq!(rider.state, RiderState::Browsing);
        assert_eq!(rider.requested_at, Some(ONE_SEC_MS), "requested_at set when transitioning to Browsing");

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("quote accepted event");
        assert_eq!(next_event.kind, EventKind::QuoteAccepted);
        assert_eq!(next_event.timestamp, 2 * ONE_SEC_MS);
        assert_eq!(next_event.subject, Some(EventSubject::Rider(rider_entity)));
    }
}
