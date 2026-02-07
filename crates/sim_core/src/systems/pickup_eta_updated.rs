use bevy_ecs::prelude::{Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Rider, RiderState, Trip, TripState};
use crate::scenario::RiderCancelConfig;

/// Pure patience check: if the projected pickup time exceeds the rider's wait
/// deadline, schedules a `RiderCancel` event at delta 0 so `rider_cancel_system`
/// handles all cancellation mutations. No direct state changes here.
pub fn pickup_eta_updated_system(
    event: Res<CurrentEvent>,
    mut clock: ResMut<SimulationClock>,
    cancel_config: Option<Res<RiderCancelConfig>>,
    trips: Query<&Trip>,
    riders: Query<&Rider>,
) {
    if event.0.kind != EventKind::PickupEtaUpdated {
        return;
    }

    let Some(EventSubject::Trip(trip_entity)) = event.0.subject else {
        return;
    };

    let Ok(trip) = trips.get(trip_entity) else {
        return;
    };
    if trip.state != TripState::EnRoute {
        return;
    }

    let rider_entity = trip.rider;
    let driver_entity = trip.driver;
    let Ok(rider) = riders.get(rider_entity) else {
        return;
    };
    if rider.state != RiderState::Waiting {
        return;
    }
    if rider.matched_driver != Some(driver_entity) {
        return;
    }

    let config = cancel_config.as_deref().copied().unwrap_or_default();
    let min_wait_ms = config.min_wait_secs.saturating_mul(1000);
    let max_wait_ms = config
        .max_wait_secs
        .max(config.min_wait_secs)
        .saturating_mul(1000);
    let wait_start = trip.matched_at;
    let now = clock.now();
    if now < wait_start.saturating_add(min_wait_ms) {
        return;
    }

    let projected_pickup = now.saturating_add(trip.pickup_eta_ms);
    let deadline = wait_start.saturating_add(max_wait_ms);
    if projected_pickup <= deadline {
        return;
    }

    // Patience exceeded — delegate cancellation to rider_cancel_system
    clock.schedule_in(
        0,
        EventKind::RiderCancel,
        Some(EventSubject::Rider(rider_entity)),
    );
}
