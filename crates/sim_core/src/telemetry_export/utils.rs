use std::error::Error;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow::array::ArrayRef;
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;

use crate::telemetry::{DriverState, RiderState, TripState};

pub(super) const AGENT_RIDER: u8 = 0;
pub(super) const AGENT_DRIVER: u8 = 1;

pub(super) fn u64_field(name: &'static str) -> Field {
    Field::new(name, DataType::UInt64, false)
}

pub(super) fn nullable_u64_field(name: &'static str) -> Field {
    Field::new(name, DataType::UInt64, true)
}

pub(super) fn u8_field(name: &'static str) -> Field {
    Field::new(name, DataType::UInt8, false)
}

pub(super) fn f64_field(name: &'static str) -> Field {
    Field::new(name, DataType::Float64, false)
}

pub(super) fn nullable_f64_field(name: &'static str) -> Field {
    Field::new(name, DataType::Float64, true)
}

pub(super) fn write_record_batch<P: AsRef<Path>>(
    path: P,
    schema: Schema,
    arrays: Vec<ArrayRef>,
) -> Result<(), Box<dyn Error>> {
    let schema = Arc::new(schema);
    let batch = RecordBatch::try_new(schema.clone(), arrays)?;
    let file = File::create(path)?;
    let mut writer = ArrowWriter::try_new(file, schema, None)?;
    writer.write(&batch)?;
    writer.close()?;
    Ok(())
}

pub(super) fn cell_to_u64(cell: h3o::CellIndex) -> u64 {
    cell.into()
}

pub(super) fn rider_state_code(state: RiderState) -> u8 {
    match state {
        RiderState::Browsing => 0,
        RiderState::Waiting => 1,
        RiderState::InTransit => 2,
        RiderState::Completed => 3,
        RiderState::Cancelled => 4,
    }
}

pub(super) fn driver_state_code(state: DriverState) -> u8 {
    match state {
        DriverState::Idle => 0,
        DriverState::Evaluating => 1,
        DriverState::EnRoute => 2,
        DriverState::OnTrip => 3,
        DriverState::OffDuty => 4,
    }
}

pub(super) fn trip_state_code(state: TripState) -> u8 {
    match state {
        TripState::EnRoute => 0,
        TripState::OnTrip => 1,
        TripState::Completed => 2,
        TripState::Cancelled => 3,
    }
}
