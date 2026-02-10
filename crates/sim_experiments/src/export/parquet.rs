use std::sync::Arc;

use arrow::array::{ArrayRef, Float64Array, UInt64Array};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;

use crate::metrics::SimulationResult;

pub(crate) fn export_to_parquet_impl(
    results: &[SimulationResult],
    file: std::fs::File,
) -> Result<(), Box<dyn std::error::Error>> {
    let batch = build_record_batch(results)?;
    let props = WriterProperties::builder().build();
    let mut writer = ArrowWriter::try_new(file, batch.schema(), Some(props))?;
    writer.write(&batch)?;
    writer.close()?;

    Ok(())
}

fn build_record_batch(
    results: &[SimulationResult],
) -> Result<RecordBatch, arrow::error::ArrowError> {
    let schema = Arc::new(parquet_schema());
    let arrays = build_arrays(results);

    RecordBatch::try_new(schema, arrays)
}

fn parquet_schema() -> Schema {
    Schema::new(vec![
        Field::new("total_riders", DataType::UInt64, false),
        Field::new("completed_riders", DataType::UInt64, false),
        Field::new("abandoned_quote_riders", DataType::UInt64, false),
        Field::new("cancelled_riders", DataType::UInt64, false),
        Field::new("conversion_rate", DataType::Float64, false),
        Field::new("platform_revenue", DataType::Float64, false),
        Field::new("driver_payouts", DataType::Float64, false),
        Field::new("total_fares_collected", DataType::Float64, false),
        Field::new("avg_time_to_match_ms", DataType::Float64, false),
        Field::new("median_time_to_match_ms", DataType::Float64, false),
        Field::new("p90_time_to_match_ms", DataType::Float64, false),
        Field::new("avg_time_to_pickup_ms", DataType::Float64, false),
        Field::new("median_time_to_pickup_ms", DataType::Float64, false),
        Field::new("p90_time_to_pickup_ms", DataType::Float64, false),
        Field::new("completed_trips", DataType::UInt64, false),
        Field::new("riders_abandoned_price", DataType::UInt64, false),
        Field::new("riders_abandoned_eta", DataType::UInt64, false),
        Field::new("riders_abandoned_stochastic", DataType::UInt64, false),
    ])
}

fn build_arrays(results: &[SimulationResult]) -> Vec<ArrayRef> {
    vec![
        Arc::new(UInt64Array::from(
            results
                .iter()
                .map(|r| r.total_riders as u64)
                .collect::<Vec<_>>(),
        )),
        Arc::new(UInt64Array::from(
            results
                .iter()
                .map(|r| r.completed_riders as u64)
                .collect::<Vec<_>>(),
        )),
        Arc::new(UInt64Array::from(
            results
                .iter()
                .map(|r| r.abandoned_quote_riders as u64)
                .collect::<Vec<_>>(),
        )),
        Arc::new(UInt64Array::from(
            results
                .iter()
                .map(|r| r.cancelled_riders as u64)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Float64Array::from(
            results
                .iter()
                .map(|r| r.conversion_rate)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Float64Array::from(
            results
                .iter()
                .map(|r| r.platform_revenue)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Float64Array::from(
            results.iter().map(|r| r.driver_payouts).collect::<Vec<_>>(),
        )),
        Arc::new(Float64Array::from(
            results
                .iter()
                .map(|r| r.total_fares_collected)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Float64Array::from(
            results
                .iter()
                .map(|r| r.avg_time_to_match_ms)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Float64Array::from(
            results
                .iter()
                .map(|r| r.median_time_to_match_ms)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Float64Array::from(
            results
                .iter()
                .map(|r| r.p90_time_to_match_ms)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Float64Array::from(
            results
                .iter()
                .map(|r| r.avg_time_to_pickup_ms)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Float64Array::from(
            results
                .iter()
                .map(|r| r.median_time_to_pickup_ms)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Float64Array::from(
            results
                .iter()
                .map(|r| r.p90_time_to_pickup_ms)
                .collect::<Vec<_>>(),
        )),
        Arc::new(UInt64Array::from(
            results
                .iter()
                .map(|r| r.completed_trips as u64)
                .collect::<Vec<_>>(),
        )),
        Arc::new(UInt64Array::from(
            results
                .iter()
                .map(|r| r.riders_abandoned_price as u64)
                .collect::<Vec<_>>(),
        )),
        Arc::new(UInt64Array::from(
            results
                .iter()
                .map(|r| r.riders_abandoned_eta as u64)
                .collect::<Vec<_>>(),
        )),
        Arc::new(UInt64Array::from(
            results
                .iter()
                .map(|r| r.riders_abandoned_stochastic as u64)
                .collect::<Vec<_>>(),
        )),
    ]
}
