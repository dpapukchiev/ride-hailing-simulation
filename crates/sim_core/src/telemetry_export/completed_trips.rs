use std::error::Error;
use std::path::Path;
use std::sync::Arc;

use arrow::array::{ArrayRef, UInt64Array};
use arrow::datatypes::Schema;

use crate::telemetry::SimTelemetry;

use super::utils::{u64_field, write_record_batch};

pub fn write_completed_trips_parquet<P: AsRef<Path>>(
    path: P,
    telemetry: &SimTelemetry,
) -> Result<(), Box<dyn Error>> {
    let mut trip_entities = Vec::with_capacity(telemetry.completed_trips.len());
    let mut rider_entities = Vec::with_capacity(telemetry.completed_trips.len());
    let mut driver_entities = Vec::with_capacity(telemetry.completed_trips.len());
    let mut completed_at = Vec::with_capacity(telemetry.completed_trips.len());
    let mut requested_at = Vec::with_capacity(telemetry.completed_trips.len());
    let mut matched_at = Vec::with_capacity(telemetry.completed_trips.len());
    let mut pickup_at = Vec::with_capacity(telemetry.completed_trips.len());

    for record in &telemetry.completed_trips {
        trip_entities.push(record.trip_entity.to_bits());
        rider_entities.push(record.rider_entity.to_bits());
        driver_entities.push(record.driver_entity.to_bits());
        completed_at.push(record.completed_at);
        requested_at.push(record.requested_at);
        matched_at.push(record.matched_at);
        pickup_at.push(record.pickup_at);
    }

    let schema = Schema::new(vec![
        u64_field("trip_entity"),
        u64_field("rider_entity"),
        u64_field("driver_entity"),
        u64_field("completed_at"),
        u64_field("requested_at"),
        u64_field("matched_at"),
        u64_field("pickup_at"),
    ]);

    let arrays: Vec<ArrayRef> = vec![
        Arc::new(UInt64Array::from(trip_entities)),
        Arc::new(UInt64Array::from(rider_entities)),
        Arc::new(UInt64Array::from(driver_entities)),
        Arc::new(UInt64Array::from(completed_at)),
        Arc::new(UInt64Array::from(requested_at)),
        Arc::new(UInt64Array::from(matched_at)),
        Arc::new(UInt64Array::from(pickup_at)),
    ];

    write_record_batch(path, schema, arrays)
}
