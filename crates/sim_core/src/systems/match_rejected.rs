use bevy_ecs::prelude::{Entity, Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Rider, Waiting};
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
    mut riders: Query<(Entity, &mut Rider, Option<&Waiting>)>,
) {
    if event.0.kind != EventKind::MatchRejected {
        return;
    }

    let Some(EventSubject::Rider(rider_entity)) = event.0.subject else {
        return;
    };

    let Ok((_entity, mut rider, waiting)) = riders.get_mut(rider_entity) else {
        return;
    };

    rider.matched_driver = None;

    // Only schedule per-rider TryMatch when batch matching is disabled
    let batch_enabled = batch_config.as_deref().is_some_and(|c| c.enabled);
    if waiting.is_some() && !batch_enabled {
        clock.schedule_in_secs(
            MATCH_RETRY_SECS,
            EventKind::TryMatch,
            Some(EventSubject::Rider(rider_entity)),
        );
    }
}
