use bevy_ecs::prelude::{Commands, Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{
    Driver, DriverEarnings, DriverStateCommands, InTransit, OnTrip, Rider, RiderCompleted, Trip,
    TripCompleted, TripFinancials, TripOnTrip, TripTiming,
};
use crate::pricing::{
    calculate_driver_earnings, calculate_platform_revenue, calculate_trip_fare_with_config,
    PricingConfig,
};
use crate::telemetry::{CompletedTripRecord, SimTelemetry};

#[allow(clippy::too_many_arguments)]
pub fn trip_completed_system(
    event: Res<CurrentEvent>,
    mut clock: ResMut<SimulationClock>,
    pricing_config: Res<PricingConfig>,
    mut telemetry: ResMut<SimTelemetry>,
    mut commands: Commands,
    mut trips: Query<(
        &mut Trip,
        &mut TripTiming,
        &TripFinancials,
        Option<&TripOnTrip>,
    )>,
    mut riders: Query<(&mut Rider, Option<&InTransit>)>,
    mut drivers: Query<(&mut Driver, Option<&OnTrip>)>,
    mut driver_earnings: Query<&mut DriverEarnings>,
) {
    if event.0.kind != EventKind::TripCompleted {
        return;
    }

    let Some(EventSubject::Trip(trip_entity)) = event.0.subject else {
        return;
    };

    let Ok((trip, mut timing, financials, on_trip)) = trips.get_mut(trip_entity) else {
        return;
    };
    if on_trip.is_none() {
        return;
    }

    let driver_entity = trip.driver;
    let rider_entity = trip.rider;

    // Use agreed fare (quoted at accept, may include surge) or fall back to current pricing
    let fare = financials.agreed_fare.unwrap_or_else(|| {
        calculate_trip_fare_with_config(trip.pickup, trip.dropoff, *pricing_config)
    });

    // Calculate base fare (without surge) to determine surge impact
    let base_fare = calculate_trip_fare_with_config(trip.pickup, trip.dropoff, *pricing_config);
    let surge_impact = (fare - base_fare).max(0.0); // Ensure non-negative

    let commission = calculate_platform_revenue(fare, pricing_config.commission_rate);
    let driver_earnings_amount = calculate_driver_earnings(fare, pricing_config.commission_rate);

    // Update driver state and clear trip backlink
    if let Ok((mut driver, on_trip)) = drivers.get_mut(driver_entity) {
        if on_trip.is_some() {
            commands.entity(driver_entity).set_driver_state_idle();
        }
        driver.matched_rider = None;
        driver.assigned_trip = None;
    }

    // Update earnings
    if let Ok(mut earnings) = driver_earnings.get_mut(driver_entity) {
        earnings.daily_earnings += driver_earnings_amount;
    }

    // Delegate offduty threshold check to driver_offduty_check_system via targeted event
    clock.schedule_in(
        0,
        EventKind::CheckDriverOffDuty,
        Some(EventSubject::Driver(driver_entity)),
    );

    if let Ok((mut rider, in_transit)) = riders.get_mut(rider_entity) {
        if in_transit.is_some() {
            commands
                .entity(rider_entity)
                .remove::<InTransit>()
                .insert(RiderCompleted);
        }
        rider.matched_driver = None;
    }

    let completed_at = clock.now();
    let pickup_at = timing.pickup_at.unwrap_or(completed_at);
    timing.dropoff_at = Some(completed_at);
    commands
        .entity(trip_entity)
        .remove::<TripOnTrip>()
        .insert(TripCompleted);

    telemetry.completed_trips.push(CompletedTripRecord {
        trip_entity,
        rider_entity,
        driver_entity,
        completed_at,
        requested_at: timing.requested_at,
        matched_at: timing.matched_at,
        pickup_at,
        fare,
        surge_impact,
    });
    telemetry.riders_completed_total = telemetry.riders_completed_total.saturating_add(1);
    telemetry.platform_revenue_total += commission;
    telemetry.total_fares_collected += fare;

    commands.entity(rider_entity).despawn();
}
