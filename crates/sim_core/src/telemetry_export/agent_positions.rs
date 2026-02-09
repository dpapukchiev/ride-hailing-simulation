use std::error::Error;
use std::path::Path;
use std::sync::Arc;

use arrow::array::{ArrayRef, Float64Array, UInt64Array, UInt8Array};
use arrow::datatypes::Schema;

use crate::telemetry::SimSnapshots;

use super::utils::{
    cell_to_u64, driver_state_code, nullable_f64_field, rider_state_code, u64_field, u8_field,
    write_record_batch, AGENT_DRIVER, AGENT_RIDER,
};

pub fn write_agent_positions_parquet<P: AsRef<Path>>(
    path: P,
    snapshots: &SimSnapshots,
) -> Result<(), Box<dyn Error>> {
    let mut timestamp_ms = Vec::new();
    let mut entity = Vec::new();
    let mut agent_type = Vec::new();
    let mut state = Vec::new();
    let mut cell = Vec::new();
    let mut lat = Vec::new();
    let mut lng = Vec::new();

    for snapshot in &snapshots.snapshots {
        for rider in &snapshot.riders {
            timestamp_ms.push(snapshot.timestamp_ms);
            entity.push(rider.entity.to_bits());
            agent_type.push(AGENT_RIDER);
            state.push(rider_state_code(rider.state));
            cell.push(cell_to_u64(rider.cell));
            if let Some(geo) = rider.geo {
                lat.push(Some(geo.lat));
                lng.push(Some(geo.lng));
            } else {
                lat.push(None);
                lng.push(None);
            }
        }
        for driver in &snapshot.drivers {
            timestamp_ms.push(snapshot.timestamp_ms);
            entity.push(driver.entity.to_bits());
            agent_type.push(AGENT_DRIVER);
            state.push(driver_state_code(driver.state));
            cell.push(cell_to_u64(driver.cell));
            if let Some(geo) = driver.geo {
                lat.push(Some(geo.lat));
                lng.push(Some(geo.lng));
            } else {
                lat.push(None);
                lng.push(None);
            }
        }
    }

    let schema = Schema::new(vec![
        u64_field("timestamp_ms"),
        u64_field("entity"),
        u8_field("agent_type"),
        u8_field("state"),
        u64_field("cell"),
        nullable_f64_field("lat"),
        nullable_f64_field("lng"),
    ]);

    let arrays: Vec<ArrayRef> = vec![
        Arc::new(UInt64Array::from(timestamp_ms)),
        Arc::new(UInt64Array::from(entity)),
        Arc::new(UInt8Array::from(agent_type)),
        Arc::new(UInt8Array::from(state)),
        Arc::new(UInt64Array::from(cell)),
        Arc::new(Float64Array::from_iter(lat)),
        Arc::new(Float64Array::from_iter(lng)),
    ];

    write_record_batch(path, schema, arrays)
}
