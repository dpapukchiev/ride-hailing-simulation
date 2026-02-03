use std::error::Error;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow::array::{ArrayRef, UInt64Array, UInt8Array};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;

use crate::ecs::{DriverState, RiderState};
use crate::telemetry::{SimSnapshots, SimTelemetry};

const AGENT_RIDER: u8 = 0;
const AGENT_DRIVER: u8 = 1;

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
        Field::new("trip_entity", DataType::UInt64, false),
        Field::new("rider_entity", DataType::UInt64, false),
        Field::new("driver_entity", DataType::UInt64, false),
        Field::new("completed_at", DataType::UInt64, false),
        Field::new("requested_at", DataType::UInt64, false),
        Field::new("matched_at", DataType::UInt64, false),
        Field::new("pickup_at", DataType::UInt64, false),
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
        Field::new("timestamp_ms", DataType::UInt64, false),
        Field::new("riders_browsing", DataType::UInt64, false),
        Field::new("riders_waiting", DataType::UInt64, false),
        Field::new("riders_in_transit", DataType::UInt64, false),
        Field::new("riders_completed", DataType::UInt64, false),
        Field::new("riders_cancelled", DataType::UInt64, false),
        Field::new("drivers_idle", DataType::UInt64, false),
        Field::new("drivers_evaluating", DataType::UInt64, false),
        Field::new("drivers_en_route", DataType::UInt64, false),
        Field::new("drivers_on_trip", DataType::UInt64, false),
        Field::new("drivers_off_duty", DataType::UInt64, false),
        Field::new("trips_en_route", DataType::UInt64, false),
        Field::new("trips_on_trip", DataType::UInt64, false),
        Field::new("trips_completed", DataType::UInt64, false),
        Field::new("trips_cancelled", DataType::UInt64, false),
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

pub fn write_agent_positions_parquet<P: AsRef<Path>>(
    path: P,
    snapshots: &SimSnapshots,
) -> Result<(), Box<dyn Error>> {
    let mut timestamp_ms = Vec::new();
    let mut entity = Vec::new();
    let mut agent_type = Vec::new();
    let mut state = Vec::new();
    let mut cell = Vec::new();

    for snapshot in &snapshots.snapshots {
        for rider in &snapshot.riders {
            timestamp_ms.push(snapshot.timestamp_ms);
            entity.push(rider.entity.to_bits());
            agent_type.push(AGENT_RIDER);
            state.push(rider_state_code(rider.state));
            cell.push(cell_to_u64(rider.cell));
        }
        for driver in &snapshot.drivers {
            timestamp_ms.push(snapshot.timestamp_ms);
            entity.push(driver.entity.to_bits());
            agent_type.push(AGENT_DRIVER);
            state.push(driver_state_code(driver.state));
            cell.push(cell_to_u64(driver.cell));
        }
    }

    let schema = Schema::new(vec![
        Field::new("timestamp_ms", DataType::UInt64, false),
        Field::new("entity", DataType::UInt64, false),
        Field::new("agent_type", DataType::UInt8, false),
        Field::new("state", DataType::UInt8, false),
        Field::new("cell", DataType::UInt64, false),
    ]);

    let arrays: Vec<ArrayRef> = vec![
        Arc::new(UInt64Array::from(timestamp_ms)),
        Arc::new(UInt64Array::from(entity)),
        Arc::new(UInt8Array::from(agent_type)),
        Arc::new(UInt8Array::from(state)),
        Arc::new(UInt64Array::from(cell)),
    ];

    write_record_batch(path, schema, arrays)
}

fn write_record_batch<P: AsRef<Path>>(
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

fn cell_to_u64(cell: h3o::CellIndex) -> u64 {
    cell.into()
}

fn rider_state_code(state: RiderState) -> u8 {
    match state {
        RiderState::Browsing => 0,
        RiderState::Waiting => 1,
        RiderState::InTransit => 2,
        RiderState::Completed => 3,
        RiderState::Cancelled => 4,
    }
}

fn driver_state_code(state: DriverState) -> u8 {
    match state {
        DriverState::Idle => 0,
        DriverState::Evaluating => 1,
        DriverState::EnRoute => 2,
        DriverState::OnTrip => 3,
        DriverState::OffDuty => 4,
    }
}
