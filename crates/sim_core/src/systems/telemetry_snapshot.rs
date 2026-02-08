use bevy_ecs::prelude::{Entity, Query, Res, ResMut};

use crate::clock::SimulationClock;
use crate::ecs::{
    Browsing, Driver, DriverEarnings, DriverFatigue, EnRoute, Evaluating, Idle, InTransit, OffDuty,
    OnTrip, Position, Rider, RiderCancelled, RiderCompleted, Trip, TripCancelled, TripCompleted,
    TripEnRoute, TripFinancials, TripOnTrip, TripTiming, Waiting,
};
use crate::telemetry::{
    DriverSnapshot, DriverState, RiderSnapshot, RiderState, SimCounts, SimSnapshot,
    SimSnapshotConfig, SimSnapshots, SimTelemetry, TripSnapshot, TripState,
};

fn rider_state_from_markers(
    browsing: Option<&Browsing>,
    waiting: Option<&Waiting>,
    in_transit: Option<&InTransit>,
    completed: Option<&RiderCompleted>,
    cancelled: Option<&RiderCancelled>,
) -> RiderState {
    if browsing.is_some() {
        RiderState::Browsing
    } else if waiting.is_some() {
        RiderState::Waiting
    } else if in_transit.is_some() {
        RiderState::InTransit
    } else if completed.is_some() {
        RiderState::Completed
    } else if cancelled.is_some() {
        RiderState::Cancelled
    } else {
        RiderState::Browsing
    }
}

fn driver_state_from_markers(
    idle: Option<&Idle>,
    evaluating: Option<&Evaluating>,
    en_route: Option<&EnRoute>,
    on_trip: Option<&OnTrip>,
    off_duty: Option<&OffDuty>,
) -> DriverState {
    if idle.is_some() {
        DriverState::Idle
    } else if evaluating.is_some() {
        DriverState::Evaluating
    } else if en_route.is_some() {
        DriverState::EnRoute
    } else if on_trip.is_some() {
        DriverState::OnTrip
    } else if off_duty.is_some() {
        DriverState::OffDuty
    } else {
        DriverState::Idle
    }
}

fn trip_state_from_markers(
    en_route: Option<&TripEnRoute>,
    on_trip: Option<&TripOnTrip>,
    completed: Option<&TripCompleted>,
    cancelled: Option<&TripCancelled>,
) -> TripState {
    if en_route.is_some() {
        TripState::EnRoute
    } else if on_trip.is_some() {
        TripState::OnTrip
    } else if completed.is_some() {
        TripState::Completed
    } else if cancelled.is_some() {
        TripState::Cancelled
    } else {
        TripState::EnRoute
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn capture_snapshot_system(
    clock: Res<SimulationClock>,
    config: Res<SimSnapshotConfig>,
    mut snapshots: ResMut<SimSnapshots>,
    telemetry: Res<SimTelemetry>,
    rider_query: Query<(
        Entity,
        &Rider,
        &Position,
        Option<&Browsing>,
        Option<&Waiting>,
        Option<&InTransit>,
        Option<&RiderCompleted>,
        Option<&RiderCancelled>,
    )>,
    driver_query: Query<(
        Entity,
        &Driver,
        &Position,
        Option<&Idle>,
        Option<&Evaluating>,
        Option<&EnRoute>,
        Option<&OnTrip>,
        Option<&OffDuty>,
    )>,
    driver_earnings_query: Query<&DriverEarnings>,
    driver_fatigue_query: Query<&DriverFatigue>,
    trip_query: Query<(
        Entity,
        &Trip,
        &TripTiming,
        &TripFinancials,
        Option<&TripEnRoute>,
        Option<&TripOnTrip>,
        Option<&TripCompleted>,
        Option<&TripCancelled>,
    )>,
) {
    // Interval check is now done via schedule condition, but keep for safety
    let now = clock.now();
    let should_capture = match snapshots.last_snapshot_at {
        None => true,
        Some(last) => now.saturating_sub(last) >= config.interval_ms,
    };
    if !should_capture {
        return;
    }

    let mut counts = SimCounts {
        riders_cancelled_total: telemetry.riders_cancelled_total,
        riders_completed_total: telemetry.riders_completed_total,
        riders_abandoned_quote_total: telemetry.riders_abandoned_quote_total,
        ..Default::default()
    };

    // Remove double iteration: collect riders in single pass
    let mut riders = Vec::new();
    for (entity, rider, position, browsing, waiting, in_transit, completed, cancelled) in
        rider_query.iter()
    {
        let state = rider_state_from_markers(browsing, waiting, in_transit, completed, cancelled);
        counts.add_rider(state);
        riders.push(RiderSnapshot {
            entity,
            cell: position.0,
            state,
            matched_driver: rider.matched_driver,
        });
    }

    // Remove double iteration: collect drivers in single pass
    let mut drivers = Vec::new();
    for (entity, _driver, position, idle, evaluating, en_route, on_trip, off_duty) in
        driver_query.iter()
    {
        let state = driver_state_from_markers(idle, evaluating, en_route, on_trip, off_duty);
        counts.add_driver(state);
        let earnings = driver_earnings_query.get(entity).ok().copied();
        let fatigue = driver_fatigue_query.get(entity).ok().copied();
        drivers.push(DriverSnapshot {
            entity,
            cell: position.0,
            state,
            daily_earnings: earnings.map(|e| e.daily_earnings),
            daily_earnings_target: earnings.map(|e| e.daily_earnings_target),
            session_start_time_ms: earnings.map(|e| e.session_start_time_ms),
            session_end_time_ms: earnings.and_then(|e| e.session_end_time_ms),
            fatigue_threshold_ms: fatigue.map(|f| f.fatigue_threshold_ms),
        });
    }

    // Remove double iteration: collect trips in single pass
    let mut trips = Vec::new();
    for (entity, trip, timing, financials, tr_en_route, tr_on_trip, tr_completed, tr_cancelled) in
        trip_query.iter()
    {
        let state = trip_state_from_markers(tr_en_route, tr_on_trip, tr_completed, tr_cancelled);
        counts.add_trip(state);
        trips.push(TripSnapshot {
            entity,
            rider: trip.rider,
            driver: trip.driver,
            state,
            pickup_cell: trip.pickup,
            dropoff_cell: trip.dropoff,
            pickup_distance_km_at_accept: financials.pickup_distance_km_at_accept,
            requested_at: timing.requested_at,
            matched_at: timing.matched_at,
            pickup_at: timing.pickup_at,
            dropoff_at: timing.dropoff_at,
            cancelled_at: timing.cancelled_at,
        });
    }

    snapshots.last_snapshot_at = Some(now);
    snapshots.snapshots.push_back(SimSnapshot {
        timestamp_ms: now,
        counts,
        riders,
        drivers,
        trips,
    });

    if snapshots.snapshots.len() > config.max_snapshots {
        snapshots.snapshots.pop_front();
    }
}
