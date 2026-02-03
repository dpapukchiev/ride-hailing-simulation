use bevy_ecs::prelude::{Commands, Query, Res};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Driver, DriverState, Rider, RiderState, Trip, TripState};
use crate::scenario::RiderCancelConfig;

pub fn pickup_eta_updated_system(
    event: Res<CurrentEvent>,
    clock: Res<SimulationClock>,
    cancel_config: Option<Res<RiderCancelConfig>>,
    mut commands: Commands,
    mut trips: Query<&mut Trip>,
    mut riders: Query<&mut Rider>,
    mut drivers: Query<&mut Driver>,
) {
    if event.0.kind != EventKind::PickupEtaUpdated {
        return;
    }

    let Some(EventSubject::Trip(trip_entity)) = event.0.subject else {
        return;
    };

    let Ok(mut trip) = trips.get_mut(trip_entity) else {
        return;
    };
    if trip.state != TripState::EnRoute {
        return;
    }

    let rider_entity = trip.rider;
    let driver_entity = trip.driver;
    let Ok(mut rider) = riders.get_mut(rider_entity) else {
        return;
    };
    if rider.state != RiderState::Waiting {
        return;
    }
    if rider.matched_driver != Some(driver_entity) {
        return;
    }

    let Ok(mut driver) = drivers.get_mut(driver_entity) else {
        return;
    };
    if driver.state != DriverState::EnRoute {
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

    trip.state = TripState::Cancelled;
    trip.cancelled_at = Some(now);
    driver.state = DriverState::Idle;
    driver.matched_rider = None;
    rider.state = RiderState::Cancelled;
    rider.matched_driver = None;
    commands.entity(rider_entity).despawn();
}
