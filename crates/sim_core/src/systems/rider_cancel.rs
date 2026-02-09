use bevy_ecs::prelude::{Commands, Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{
    Driver, DriverStateCommands, EnRoute, Evaluating, Rider, Trip, TripCancelled, TripCompleted,
    TripEnRoute, TripOnTrip, TripTiming, Waiting,
};
use crate::telemetry::SimTelemetry;

pub fn rider_cancel_system(
    event: Res<CurrentEvent>,
    clock: Res<SimulationClock>,
    mut commands: Commands,
    mut telemetry: ResMut<SimTelemetry>,
    mut riders: Query<(&mut Rider, Option<&Waiting>)>,
    mut drivers: Query<(&mut Driver, Option<&EnRoute>, Option<&Evaluating>)>,
    mut trips: Query<(&mut Trip, &mut TripTiming, Option<&TripEnRoute>)>,
) {
    if event.0.kind != EventKind::RiderCancel {
        return;
    }

    let Some(EventSubject::Rider(rider_entity)) = event.0.subject else {
        return;
    };
    let Ok((mut rider, waiting)) = riders.get_mut(rider_entity) else {
        return;
    };
    if waiting.is_none() {
        return;
    }

    if let Some(driver_entity) = rider.matched_driver {
        // Use assigned_trip for O(1) trip lookup instead of scanning all trips
        if let Some(trip_entity) = rider.assigned_trip {
            if let Ok((_trip, mut timing, en_route)) = trips.get_mut(trip_entity) {
                if en_route.is_some() {
                    timing.cancelled_at = Some(clock.now());
                    commands
                        .entity(trip_entity)
                        .remove::<TripEnRoute>()
                        .remove::<TripOnTrip>()
                        .remove::<TripCompleted>()
                        .insert(TripCancelled);
                }
            }
        }

        if let Ok((mut driver, en_route, evaluating)) = drivers.get_mut(driver_entity) {
            if driver.matched_rider == Some(rider_entity) {
                if en_route.is_some() || evaluating.is_some() {
                    commands.entity(driver_entity).set_driver_state_idle();
                }
                driver.matched_rider = None;
            }
            // Clear the trip backlink from the driver
            driver.assigned_trip = None;
        }
    }

    rider.matched_driver = None;
    rider.assigned_trip = None;
    telemetry.riders_cancelled_total = telemetry.riders_cancelled_total.saturating_add(1);
    // Track pickup timeout cancellation
    telemetry.riders_cancelled_pickup_timeout =
        telemetry.riders_cancelled_pickup_timeout.saturating_add(1);
    commands.entity(rider_entity).despawn();
}
