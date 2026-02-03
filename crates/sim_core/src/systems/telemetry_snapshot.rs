use bevy_ecs::prelude::{Entity, Query, Res, ResMut};

use crate::clock::SimulationClock;
use crate::ecs::{Driver, Position, Rider, Trip};
use crate::telemetry::{
    DriverSnapshot, RiderSnapshot, SimCounts, SimSnapshot, SimSnapshotConfig, SimSnapshots,
    TripSnapshot,
};

pub fn capture_snapshot_system(
    clock: Res<SimulationClock>,
    config: Res<SimSnapshotConfig>,
    mut snapshots: ResMut<SimSnapshots>,
    rider_query: Query<(Entity, &Rider, &Position)>,
    driver_query: Query<(Entity, &Driver, &Position)>,
    trip_query: Query<(Entity, &Trip)>,
) {
    let now = clock.now();
    let should_capture = match snapshots.last_snapshot_at {
        None => true,
        Some(last) => now.saturating_sub(last) >= config.interval_ms,
    };
    if !should_capture {
        return;
    }

    let mut counts = SimCounts::default();
    let mut riders = Vec::with_capacity(rider_query.iter().count());
    for (entity, rider, position) in rider_query.iter() {
        counts.add_rider(rider.state);
        riders.push(RiderSnapshot {
            entity,
            cell: position.0,
            state: rider.state,
        });
    }

    let mut drivers = Vec::with_capacity(driver_query.iter().count());
    for (entity, driver, position) in driver_query.iter() {
        counts.add_driver(driver.state);
        drivers.push(DriverSnapshot {
            entity,
            cell: position.0,
            state: driver.state,
        });
    }

    let mut trips = Vec::with_capacity(trip_query.iter().count());
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
