use std::error::Error;
use std::path::Path;
use std::sync::Arc;

use arrow::array::{ArrayRef, UInt64Array};
use arrow::datatypes::Schema;

use crate::telemetry::SimSnapshots;

use super::utils::{u64_field, write_record_batch};

pub fn write_snapshot_counts_parquet<P: AsRef<Path>>(
    path: P,
    snapshots: &SimSnapshots,
) -> Result<(), Box<dyn Error>> {
    let mut timestamp_ms = Vec::with_capacity(snapshots.snapshots.len());
    let mut riders_browsing = Vec::with_capacity(snapshots.snapshots.len());
    let mut riders_waiting = Vec::with_capacity(snapshots.snapshots.len());
    let mut riders_in_transit = Vec::with_capacity(snapshots.snapshots.len());
    let mut riders_completed = Vec::with_capacity(snapshots.snapshots.len());
    let mut riders_cancelled = Vec::with_capacity(snapshots.snapshots.len());
    let mut drivers_idle = Vec::with_capacity(snapshots.snapshots.len());
    let mut drivers_evaluating = Vec::with_capacity(snapshots.snapshots.len());
    let mut drivers_en_route = Vec::with_capacity(snapshots.snapshots.len());
    let mut drivers_on_trip = Vec::with_capacity(snapshots.snapshots.len());
    let mut drivers_off_duty = Vec::with_capacity(snapshots.snapshots.len());
    let mut trips_en_route = Vec::with_capacity(snapshots.snapshots.len());
    let mut trips_on_trip = Vec::with_capacity(snapshots.snapshots.len());
    let mut trips_completed = Vec::with_capacity(snapshots.snapshots.len());
    let mut trips_cancelled = Vec::with_capacity(snapshots.snapshots.len());

    for snapshot in &snapshots.snapshots {
        timestamp_ms.push(snapshot.timestamp_ms);
        riders_browsing.push(snapshot.counts.riders_browsing as u64);
        riders_waiting.push(snapshot.counts.riders_waiting as u64);
        riders_in_transit.push(snapshot.counts.riders_in_transit as u64);
        riders_completed.push(snapshot.counts.riders_completed as u64);
        riders_cancelled.push(snapshot.counts.riders_cancelled as u64);
        drivers_idle.push(snapshot.counts.drivers_idle as u64);
        drivers_evaluating.push(snapshot.counts.drivers_evaluating as u64);
        drivers_en_route.push(snapshot.counts.drivers_en_route as u64);
        drivers_on_trip.push(snapshot.counts.drivers_on_trip as u64);
        drivers_off_duty.push(snapshot.counts.drivers_off_duty as u64);
        trips_en_route.push(snapshot.counts.trips_en_route as u64);
        trips_on_trip.push(snapshot.counts.trips_on_trip as u64);
        trips_completed.push(snapshot.counts.trips_completed as u64);
        trips_cancelled.push(snapshot.counts.trips_cancelled as u64);
    }

    let schema = Schema::new(vec![
        u64_field("timestamp_ms"),
        u64_field("riders_browsing"),
        u64_field("riders_waiting"),
        u64_field("riders_in_transit"),
        u64_field("riders_completed"),
        u64_field("riders_cancelled"),
        u64_field("drivers_idle"),
        u64_field("drivers_evaluating"),
        u64_field("drivers_en_route"),
        u64_field("drivers_on_trip"),
        u64_field("drivers_off_duty"),
        u64_field("trips_en_route"),
        u64_field("trips_on_trip"),
        u64_field("trips_completed"),
        u64_field("trips_cancelled"),
    ]);

    let arrays: Vec<ArrayRef> = vec![
        Arc::new(UInt64Array::from(timestamp_ms)),
        Arc::new(UInt64Array::from(riders_browsing)),
        Arc::new(UInt64Array::from(riders_waiting)),
        Arc::new(UInt64Array::from(riders_in_transit)),
        Arc::new(UInt64Array::from(riders_completed)),
        Arc::new(UInt64Array::from(riders_cancelled)),
        Arc::new(UInt64Array::from(drivers_idle)),
        Arc::new(UInt64Array::from(drivers_evaluating)),
        Arc::new(UInt64Array::from(drivers_en_route)),
        Arc::new(UInt64Array::from(drivers_on_trip)),
        Arc::new(UInt64Array::from(drivers_off_duty)),
        Arc::new(UInt64Array::from(trips_en_route)),
        Arc::new(UInt64Array::from(trips_on_trip)),
        Arc::new(UInt64Array::from(trips_completed)),
        Arc::new(UInt64Array::from(trips_cancelled)),
    ];

    write_record_batch(path, schema, arrays)
}
