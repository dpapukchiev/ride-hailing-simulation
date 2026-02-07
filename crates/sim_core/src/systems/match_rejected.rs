use bevy_ecs::prelude::{Entity, Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Rider, RiderState};
use crate::scenario::BatchMatchingConfig;

const MATCH_RETRY_SECS: u64 = 30;

/// Handles rider-side cleanup after a driver rejects the match.
///
/// Clears the rider's `matched_driver` link and, when batch matching is disabled,
/// schedules a `TryMatch` retry so the rider can be re-matched.
pub fn match_rejected_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    batch_config: Option<Res<BatchMatchingConfig>>,
    mut riders: Query<(Entity, &mut Rider)>,
) {
    if event.0.kind != EventKind::MatchRejected {
        return;
    }

    let Some(EventSubject::Rider(rider_entity)) = event.0.subject else {
        return;
    };

    let Ok((_entity, mut rider)) = riders.get_mut(rider_entity) else {
        return;
    };

    rider.matched_driver = None;

    // Only schedule per-rider TryMatch when batch matching is disabled
    let batch_enabled = batch_config.as_deref().is_some_and(|c| c.enabled);
    if rider.state == RiderState::Waiting && !batch_enabled {
        clock.schedule_in_secs(
            MATCH_RETRY_SECS,
            EventKind::TryMatch,
            Some(EventSubject::Rider(rider_entity)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    use crate::ecs::Position;

    fn setup_world_with_rejected_rider(
        batch_config: Option<BatchMatchingConfig>,
    ) -> (World, Entity) {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        if let Some(config) = batch_config {
            world.insert_resource(config);
        }

        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
        let destination = cell
            .grid_disk::<Vec<_>>(1)
            .into_iter()
            .find(|c| *c != cell)
            .expect("neighbor cell");

        let fake_driver = world.spawn_empty().id();
        let rider_entity = world
            .spawn((
                Rider {
                    state: RiderState::Waiting,
                    matched_driver: Some(fake_driver),
                    assigned_trip: None,
                    destination: Some(destination),
                    requested_at: None,
                    quote_rejections: 0,
                    accepted_fare: Some(10.0),
                    last_rejection_reason: None,
                },
                Position(cell),
            ))
            .id();

        world.resource_mut::<SimulationClock>().schedule_at_secs(
            1,
            EventKind::MatchRejected,
            Some(EventSubject::Rider(rider_entity)),
        );
        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("match rejected event");
        world.insert_resource(CurrentEvent(event));

        (world, rider_entity)
    }

    #[test]
    fn clears_matched_driver_and_schedules_retry() {
        let (mut world, rider_entity) = setup_world_with_rejected_rider(None);

        let mut schedule = Schedule::default();
        schedule.add_systems(match_rejected_system);
        schedule.run(&mut world);

        let rider = world.entity(rider_entity).get::<Rider>().expect("rider");
        assert_eq!(rider.matched_driver, None);

        // TryMatch retry should be scheduled (no batch matching config = disabled)
        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("retry event");
        assert_eq!(next_event.kind, EventKind::TryMatch);
        assert_eq!(next_event.subject, Some(EventSubject::Rider(rider_entity)));
        assert_eq!(next_event.timestamp, 1000 + MATCH_RETRY_SECS * 1000);
    }

    #[test]
    fn no_retry_when_batch_matching_enabled() {
        let (mut world, rider_entity) =
            setup_world_with_rejected_rider(Some(BatchMatchingConfig {
                enabled: true,
                interval_secs: 5,
            }));

        let mut schedule = Schedule::default();
        schedule.add_systems(match_rejected_system);
        schedule.run(&mut world);

        let rider = world.entity(rider_entity).get::<Rider>().expect("rider");
        assert_eq!(rider.matched_driver, None);

        // No TryMatch retry when batch matching is enabled
        assert!(
            world.resource::<SimulationClock>().is_empty(),
            "no events should be scheduled when batch matching is enabled"
        );
    }
}
