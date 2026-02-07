use bevy_ecs::prelude::{Commands, Entity, Query, Res, ResMut};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Rider, RiderQuote, RiderState};
use crate::scenario::{BatchMatchingConfig, RiderCancelConfig};

pub fn quote_accepted_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    mut commands: Commands,
    batch_config: Option<Res<BatchMatchingConfig>>,
    cancel_config: Option<Res<RiderCancelConfig>>,
    mut riders: Query<(Entity, &mut Rider, &RiderQuote)>,
) {
    if event.0.kind != EventKind::QuoteAccepted {
        return;
    }

    let Some(EventSubject::Rider(rider_entity)) = event.0.subject else {
        return;
    };
    let Ok((_, mut rider, quote)) = riders.get_mut(rider_entity) else {
        return;
    };
    if rider.state == RiderState::Browsing {
        rider.accepted_fare = Some(quote.fare);
        rider.state = RiderState::Waiting;
        commands.entity(rider_entity).remove::<RiderQuote>();
    }

    // Only schedule per-rider TryMatch when batch matching is disabled
    let batch_enabled = batch_config.as_deref().is_some_and(|c| c.enabled);
    if !batch_enabled {
        clock.schedule_in_secs(
            1,
            EventKind::TryMatch,
            Some(EventSubject::Rider(rider_entity)),
        );
    }

    let config = cancel_config.as_deref().copied().unwrap_or_default();
    let min_wait_secs = config.min_wait_secs;
    let max_wait_secs = config.max_wait_secs.max(config.min_wait_secs);

    // Sample cancellation time from uniform distribution between min and max
    // Use rider entity ID to ensure each rider gets a different sample even with the same seed
    let seed = config.seed.wrapping_add(rider_entity.index() as u64);
    let mut rng = StdRng::seed_from_u64(seed);
    let wait_secs = rng.gen_range(min_wait_secs..=max_wait_secs);

    clock.schedule_in_secs(
        wait_secs,
        EventKind::RiderCancel,
        Some(EventSubject::Rider(rider_entity)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    #[test]
    fn quote_accepted_transitions_rider_state() {
        use crate::test_helpers::test_neighbor_cell;

        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(RiderCancelConfig::default());
        let destination = test_neighbor_cell();

        let rider_entity = world
            .spawn((
                Rider {
                    state: RiderState::Browsing,
                    matched_driver: None,
                    destination: Some(destination),
                    requested_at: None,
                    quote_rejections: 0,
                    accepted_fare: None,
                    last_rejection_reason: None,
                },
                RiderQuote {
                    fare: 12.5,
                    eta_ms: 60_000,
                },
            ))
            .id();

        world.resource_mut::<SimulationClock>().schedule_at_secs(
            1,
            EventKind::QuoteAccepted,
            Some(EventSubject::Rider(rider_entity)),
        );

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
        // Cancellation time should be sampled from uniform distribution between min and max
        // Default config: min_wait_secs=120, max_wait_secs=2400
        // Quote accepted at 1000ms, so cancellation should be between 1000+120*1000 and 1000+2400*1000
        let config = world.resource::<RiderCancelConfig>();
        let min_timestamp = 1000 + config.min_wait_secs * 1000;
        let max_timestamp = 1000 + config.max_wait_secs * 1000;
        assert!(
            cancel_event.timestamp >= min_timestamp && cancel_event.timestamp <= max_timestamp,
            "Cancellation timestamp {} should be between {} and {}",
            cancel_event.timestamp,
            min_timestamp,
            max_timestamp
        );
        assert_eq!(
            cancel_event.subject,
            Some(EventSubject::Rider(rider_entity))
        );
    }
}
