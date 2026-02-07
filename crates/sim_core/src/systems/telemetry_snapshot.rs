use bevy_ecs::prelude::{Entity, Query, Res, ResMut};

use crate::clock::SimulationClock;
use crate::ecs::{Driver, DriverEarnings, DriverFatigue, Position, Rider, Trip};
use crate::telemetry::{
    DriverSnapshot, RiderSnapshot, SimCounts, SimSnapshot, SimSnapshotConfig, SimSnapshots,
    SimTelemetry, TripSnapshot,
};

pub fn capture_snapshot_system(
    clock: Res<SimulationClock>,
    config: Res<SimSnapshotConfig>,
    mut snapshots: ResMut<SimSnapshots>,
    telemetry: Res<SimTelemetry>,
    rider_query: Query<(Entity, &Rider, &Position)>,
    driver_query: Query<(Entity, &Driver, &Position)>,
    driver_earnings_query: Query<&DriverEarnings>,
    driver_fatigue_query: Query<&DriverFatigue>,
    trip_query: Query<(Entity, &Trip)>,
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

    let mut counts = SimCounts::default();
    counts.riders_cancelled_total = telemetry.riders_cancelled_total;
    counts.riders_completed_total = telemetry.riders_completed_total;
    counts.riders_abandoned_quote_total = telemetry.riders_abandoned_quote_total;
    
    // Remove double iteration: collect riders in single pass
    let mut riders = Vec::new();
    for (entity, rider, position) in rider_query.iter() {
        counts.add_rider(rider.state);
        riders.push(RiderSnapshot {
            entity,
            cell: position.0,
            state: rider.state,
            matched_driver: rider.matched_driver,
        });
    }

    // Remove double iteration: collect drivers in single pass
    let mut drivers = Vec::new();
    for (entity, driver, position) in driver_query.iter() {
        counts.add_driver(driver.state);
        let earnings = driver_earnings_query.get(entity).ok().copied();
        let fatigue = driver_fatigue_query.get(entity).ok().copied();
        drivers.push(DriverSnapshot {
            entity,
            cell: position.0,
            state: driver.state,
            daily_earnings: earnings.map(|e| e.daily_earnings),
            daily_earnings_target: earnings.map(|e| e.daily_earnings_target),
            session_start_time_ms: earnings.map(|e| e.session_start_time_ms),
            fatigue_threshold_ms: fatigue.map(|f| f.fatigue_threshold_ms),
        });
    }

    // Remove double iteration: collect trips in single pass
    let mut trips = Vec::new();
    for (entity, trip) in trip_query.iter() {
        counts.add_trip(trip.state);
        trips.push(TripSnapshot {
            entity,
            rider: trip.rider,
            driver: trip.driver,
            state: trip.state,
            pickup_cell: trip.pickup,
            dropoff_cell: trip.dropoff,
            pickup_distance_km_at_accept: trip.pickup_distance_km_at_accept,
            requested_at: trip.requested_at,
            matched_at: trip.matched_at,
            pickup_at: trip.pickup_at,
            dropoff_at: trip.dropoff_at,
            cancelled_at: trip.cancelled_at,
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
