use bevy_ecs::prelude::{Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Rider, RiderState};
use crate::scenario::RiderCancelConfig;

pub fn quote_accepted_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    cancel_config: Option<Res<RiderCancelConfig>>,
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

    clock.schedule_in_secs(1, EventKind::TryMatch, Some(EventSubject::Rider(rider_entity)));

    let config = cancel_config.as_deref().copied().unwrap_or_default();
    let max_wait_secs = config.max_wait_secs.max(config.min_wait_secs);
    clock.schedule_in_secs(max_wait_secs, EventKind::RiderCancel, Some(EventSubject::Rider(rider_entity)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    #[test]
    fn quote_accepted_transitions_rider_state() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(RiderCancelConfig::default());
        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
        let destination = cell
            .grid_disk::<Vec<_>>(1)
            .into_iter()
            .find(|c| *c != cell)
            .expect("neighbor cell");
        
        let rider_entity = world
            .spawn(Rider {
                state: RiderState::Browsing,
                matched_driver: None,
                destination: Some(destination),
                requested_at: None,
            })
            .id();

        world
            .resource_mut::<SimulationClock>()
            .schedule_at_secs(1, EventKind::QuoteAccepted, Some(EventSubject::Rider(rider_entity)));

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
        assert_eq!(next_event.timestamp, 2000);
        assert_eq!(next_event.subject, Some(EventSubject::Rider(rider_entity)));

        let cancel_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("rider cancel event");
        assert_eq!(cancel_event.kind, EventKind::RiderCancel);
        assert_eq!(cancel_event.timestamp, 2_401_000);
        assert_eq!(cancel_event.subject, Some(EventSubject::Rider(rider_entity)));
    }
}
