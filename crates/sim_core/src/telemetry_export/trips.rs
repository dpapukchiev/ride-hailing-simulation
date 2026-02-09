use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::sync::Arc;

use arrow::array::{ArrayRef, Float64Array, UInt64Array, UInt8Array};
use arrow::datatypes::Schema;

use crate::telemetry::{SimSnapshots, TripSnapshot};

use super::utils::{
    cell_to_u64, f64_field, nullable_u64_field, trip_state_code, u64_field, u8_field,
    write_record_batch,
};

/// Export all trips from snapshots (same data as shown in UI trip table).
/// Includes all trips in all states (EnRoute, OnTrip, Completed, Cancelled) with full details.
pub fn write_trips_parquet<P: AsRef<Path>>(
    path: P,
    snapshots: &SimSnapshots,
) -> Result<(), Box<dyn Error>> {
    let mut trips_map: HashMap<u64, (u64, TripSnapshot)> = HashMap::new();

    for snapshot in &snapshots.snapshots {
        for trip in &snapshot.trips {
            let trip_entity_bits = trip.entity.to_bits();
            trips_map
                .entry(trip_entity_bits)
                .and_modify(|(ts, stored_trip)| {
                    if snapshot.timestamp_ms > *ts {
                        *ts = snapshot.timestamp_ms;
                        *stored_trip = trip.clone();
                    }
                })
                .or_insert_with(|| (snapshot.timestamp_ms, trip.clone()));
        }
    }

    let mut trip_entities = Vec::with_capacity(trips_map.len());
    let mut rider_entities = Vec::with_capacity(trips_map.len());
    let mut driver_entities = Vec::with_capacity(trips_map.len());
    let mut state = Vec::with_capacity(trips_map.len());
    let mut pickup_cell = Vec::with_capacity(trips_map.len());
    let mut dropoff_cell = Vec::with_capacity(trips_map.len());
    let mut pickup_distance_km_at_accept = Vec::with_capacity(trips_map.len());
    let mut requested_at = Vec::with_capacity(trips_map.len());
    let mut matched_at = Vec::with_capacity(trips_map.len());
    let mut pickup_at = Vec::with_capacity(trips_map.len());
    let mut dropoff_at = Vec::with_capacity(trips_map.len());
    let mut cancelled_at = Vec::with_capacity(trips_map.len());

    for (_, trip) in trips_map.values() {
        trip_entities.push(trip.entity.to_bits());
        rider_entities.push(trip.rider.to_bits());
        driver_entities.push(trip.driver.to_bits());
        state.push(trip_state_code(trip.state));
        pickup_cell.push(cell_to_u64(trip.pickup_cell));
        dropoff_cell.push(cell_to_u64(trip.dropoff_cell));
        pickup_distance_km_at_accept.push(trip.pickup_distance_km_at_accept);
        requested_at.push(trip.requested_at);
        matched_at.push(trip.matched_at);
        pickup_at.push(trip.pickup_at);
        dropoff_at.push(trip.dropoff_at);
        cancelled_at.push(trip.cancelled_at);
    }

    let schema = Schema::new(vec![
        u64_field("trip_entity"),
        u64_field("rider_entity"),
        u64_field("driver_entity"),
        u8_field("state"),
        u64_field("pickup_cell"),
        u64_field("dropoff_cell"),
        f64_field("pickup_distance_km_at_accept"),
        u64_field("requested_at"),
        u64_field("matched_at"),
        nullable_u64_field("pickup_at"),
        nullable_u64_field("dropoff_at"),
        nullable_u64_field("cancelled_at"),
    ]);

    let arrays: Vec<ArrayRef> = vec![
        Arc::new(UInt64Array::from(trip_entities)),
        Arc::new(UInt64Array::from(rider_entities)),
        Arc::new(UInt64Array::from(driver_entities)),
        Arc::new(UInt8Array::from(state)),
        Arc::new(UInt64Array::from(pickup_cell)),
        Arc::new(UInt64Array::from(dropoff_cell)),
        Arc::new(Float64Array::from(pickup_distance_km_at_accept)),
        Arc::new(UInt64Array::from(requested_at)),
        Arc::new(UInt64Array::from(matched_at)),
        Arc::new(UInt64Array::from_iter(pickup_at.iter().copied())),
        Arc::new(UInt64Array::from_iter(dropoff_at.iter().copied())),
        Arc::new(UInt64Array::from_iter(cancelled_at.iter().copied())),
    ];

    write_record_batch(path, schema, arrays)
}
