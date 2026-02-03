use bevy_ecs::prelude::{Commands, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Position, Rider, RiderState};
use crate::scenario::PendingRiders;

pub fn request_inbound_system(
    mut commands: Commands,
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    mut pending_riders: ResMut<PendingRiders>,
) {
    if event.0.kind != EventKind::RequestInbound {
        return;
    }

    // Pop the next pending rider and spawn them just-in-time
    let Some(pending) = pending_riders.0.pop_front() else {
        return;
    };

    let rider_entity = commands
        .spawn((
            Rider {
                state: RiderState::Browsing,
                matched_driver: None,
                destination: Some(pending.destination),
                requested_at: Some(clock.now()),
            },
            Position(pending.position),
        ))
        .id();

    clock.schedule_in_secs(1, EventKind::QuoteAccepted, Some(EventSubject::Rider(rider_entity)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    use crate::clock::ONE_SEC_MS;
    use crate::scenario::PendingRider;

    #[test]
    fn ecs_system_spawns_rider_just_in_time() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());

        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
        let destination = h3o::CellIndex::try_from(0x8a1fb46622effff).expect("destination");

        let mut pending_riders = PendingRiders::default();
        pending_riders.0.push_back(PendingRider {
            position: cell,
            destination,
            request_time_ms: 1000,
        });
        world.insert_resource(pending_riders);

        world
            .resource_mut::<SimulationClock>()
            .schedule_at_secs(1, EventKind::RequestInbound, None);

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
        assert_eq!(rider.requested_at, Some(ONE_SEC_MS), "requested_at set when spawning");
        assert_eq!(rider.destination, Some(destination));

        let pending = world.resource::<PendingRiders>();
        assert_eq!(pending.0.len(), 0, "pending rider consumed");

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("quote accepted event");
        assert_eq!(next_event.kind, EventKind::QuoteAccepted);
        assert_eq!(next_event.timestamp, 2 * ONE_SEC_MS);
    }
}
