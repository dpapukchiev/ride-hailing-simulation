mod support;

use std::fs::File;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use bevy_ecs::prelude::World;
use h3o::CellIndex;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use sim_core::runner::{run_until_empty, simulation_schedule};
use sim_core::scenario::{build_scenario, ScenarioParams};
use sim_core::telemetry::{SimSnapshots, SimTelemetry, TripSnapshot, TripState};
use sim_core::telemetry_export::{
    validate_trip_timestamp_ordering, write_completed_trips_parquet, write_trips_parquet,
};

fn temp_parquet_path(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}_{nanos}.parquet"))
}

fn parquet_field_specs(path: &PathBuf) -> Vec<(String, String, bool)> {
    let file = File::open(path).expect("parquet file should exist");
    let builder =
        ParquetRecordBatchReaderBuilder::try_new(file).expect("parquet reader should build");
    builder
        .schema()
        .fields()
        .iter()
        .map(|field| {
            (
                field.name().to_string(),
                field.data_type().to_string(),
                field.is_nullable(),
            )
        })
        .collect()
}

fn make_test_trip(
    state: TripState,
    requested_at: u64,
    matched_at: u64,
    pickup_at: Option<u64>,
    dropoff_at: Option<u64>,
    cancelled_at: Option<u64>,
) -> TripSnapshot {
    let mut world = World::new();
    let trip_entity = world.spawn_empty().id();
    let rider_entity = world.spawn_empty().id();
    let driver_entity = world.spawn_empty().id();

    let cell = CellIndex::try_from(0x8928308280fffff).expect("valid cell");
    TripSnapshot {
        entity: trip_entity,
        rider: rider_entity,
        driver: driver_entity,
        state,
        pickup_cell: cell,
        dropoff_cell: cell,
        pickup_distance_km_at_accept: 1.0,
        requested_at,
        matched_at,
        pickup_at,
        dropoff_at,
        cancelled_at,
    }
}

#[test]
fn validate_enroute_trip_timestamps() {
    let trip = make_test_trip(TripState::EnRoute, 1000, 2000, None, None, None);
    assert!(validate_trip_timestamp_ordering(&trip).is_none());

    let trip = make_test_trip(TripState::EnRoute, 2000, 1000, None, None, None);
    assert!(validate_trip_timestamp_ordering(&trip).is_some());

    let trip = make_test_trip(TripState::EnRoute, 1000, 2000, Some(3000), None, None);
    assert!(validate_trip_timestamp_ordering(&trip).is_some());
}

#[test]
fn validate_ontrip_trip_timestamps() {
    let trip = make_test_trip(TripState::OnTrip, 1000, 2000, Some(3000), None, None);
    assert!(validate_trip_timestamp_ordering(&trip).is_none());

    let trip = make_test_trip(TripState::OnTrip, 1000, 3000, Some(2000), None, None);
    assert!(validate_trip_timestamp_ordering(&trip).is_some());

    let trip = make_test_trip(TripState::OnTrip, 1000, 2000, None, None, None);
    assert!(validate_trip_timestamp_ordering(&trip).is_some());
}

#[test]
fn validate_completed_trip_timestamps() {
    let trip = make_test_trip(
        TripState::Completed,
        1000,
        2000,
        Some(3000),
        Some(4000),
        None,
    );
    assert!(validate_trip_timestamp_ordering(&trip).is_none());

    let trip = make_test_trip(
        TripState::Completed,
        1000,
        2000,
        Some(4000),
        Some(3000),
        None,
    );
    assert!(validate_trip_timestamp_ordering(&trip).is_some());

    let trip = make_test_trip(TripState::Completed, 1000, 2000, Some(3000), None, None);
    assert!(validate_trip_timestamp_ordering(&trip).is_some());
}

#[test]
fn validate_cancelled_trip_timestamps() {
    let trip = make_test_trip(TripState::Cancelled, 1000, 2000, None, None, Some(3000));
    assert!(validate_trip_timestamp_ordering(&trip).is_none());

    let trip = make_test_trip(
        TripState::Cancelled,
        1000,
        2000,
        Some(3000),
        None,
        Some(4000),
    );
    assert!(validate_trip_timestamp_ordering(&trip).is_none());

    let trip = make_test_trip(TripState::Cancelled, 1000, 3000, None, None, Some(2000));
    assert!(validate_trip_timestamp_ordering(&trip).is_some());

    let trip = make_test_trip(
        TripState::Cancelled,
        1000,
        2000,
        Some(4000),
        None,
        Some(3000),
    );
    assert!(validate_trip_timestamp_ordering(&trip).is_some());

    let trip = make_test_trip(TripState::Cancelled, 1000, 2000, None, None, None);
    assert!(validate_trip_timestamp_ordering(&trip).is_some());
}

#[test]
fn validate_all_trips_in_snapshots() {
    let mut world = World::new();
    build_scenario(
        &mut world,
        ScenarioParams {
            num_riders: 10,
            num_drivers: 5,
            ..Default::default()
        }
        .with_seed(42)
        .with_request_window_hours(1)
        .with_match_radius(5)
        .with_trip_duration_cells(5, 20),
    );

    let mut schedule = simulation_schedule();
    run_until_empty(&mut world, &mut schedule, 100_000);

    let snapshots = world.resource::<SimSnapshots>();
    let mut errors = Vec::new();

    for snapshot in &snapshots.snapshots {
        for trip in &snapshot.trips {
            if let Some(error) = validate_trip_timestamp_ordering(trip) {
                errors.push(format!(
                    "Snapshot at {}ms: {}",
                    snapshot.timestamp_ms, error
                ));
            }
        }
    }

    if !errors.is_empty() {
        panic!(
            "Found {} trip timestamp ordering errors:\n{}",
            errors.len(),
            errors.join("\n")
        );
    }
}

#[test]
fn completed_trip_export_schema_matches_expected_columns() {
    let telemetry = SimTelemetry::default();
    let path = temp_parquet_path("completed_trips_schema");

    write_completed_trips_parquet(&path, &telemetry).expect("completed trips parquet should write");

    let specs = parquet_field_specs(&path);
    assert_eq!(
        specs,
        vec![
            ("trip_entity".to_string(), "UInt64".to_string(), false),
            ("rider_entity".to_string(), "UInt64".to_string(), false),
            ("driver_entity".to_string(), "UInt64".to_string(), false),
            ("completed_at".to_string(), "UInt64".to_string(), false),
            ("requested_at".to_string(), "UInt64".to_string(), false),
            ("matched_at".to_string(), "UInt64".to_string(), false),
            ("pickup_at".to_string(), "UInt64".to_string(), false),
        ]
    );

    std::fs::remove_file(path).expect("temp parquet file should be removable");
}

#[test]
fn trip_export_schema_matches_expected_columns() {
    let snapshots = SimSnapshots::default();
    let path = temp_parquet_path("trips_schema");

    write_trips_parquet(&path, &snapshots).expect("trips parquet should write");

    let specs = parquet_field_specs(&path);
    assert_eq!(
        specs,
        vec![
            ("trip_entity".to_string(), "UInt64".to_string(), false),
            ("rider_entity".to_string(), "UInt64".to_string(), false),
            ("driver_entity".to_string(), "UInt64".to_string(), false),
            ("state".to_string(), "UInt8".to_string(), false),
            ("pickup_cell".to_string(), "UInt64".to_string(), false),
            ("dropoff_cell".to_string(), "UInt64".to_string(), false),
            (
                "pickup_distance_km_at_accept".to_string(),
                "Float64".to_string(),
                false,
            ),
            ("requested_at".to_string(), "UInt64".to_string(), false),
            ("matched_at".to_string(), "UInt64".to_string(), false),
            ("pickup_at".to_string(), "UInt64".to_string(), true),
            ("dropoff_at".to_string(), "UInt64".to_string(), true),
            ("cancelled_at".to_string(), "UInt64".to_string(), true),
        ]
    );

    std::fs::remove_file(path).expect("temp parquet file should be removable");
}
