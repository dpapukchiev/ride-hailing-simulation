use bevy_ecs::prelude::{Commands, Entity, Query, Res, ResMut};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Browsing, Rider, RiderQuote, Waiting};
use crate::scenario::{BatchMatchingConfig, RiderCancelConfig};

pub fn quote_accepted_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    mut commands: Commands,
    batch_config: Option<Res<BatchMatchingConfig>>,
    cancel_config: Option<Res<RiderCancelConfig>>,
    mut riders: Query<(Entity, &mut Rider, &RiderQuote, Option<&Browsing>)>,
) {
    if event.0.kind != EventKind::QuoteAccepted {
        return;
    }

    let Some(EventSubject::Rider(rider_entity)) = event.0.subject else {
        return;
    };
    let Ok((_, mut rider, quote, browsing)) = riders.get_mut(rider_entity) else {
        return;
    };
    if browsing.is_some() {
        rider.accepted_fare = Some(quote.fare);
        commands
            .entity(rider_entity)
            .remove::<Browsing>()
            .insert(Waiting);
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
