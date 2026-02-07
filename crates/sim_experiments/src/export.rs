//! Result export and analysis utilities.
//!
//! This module provides functions to export experiment results to Parquet and JSON,
//! and to find optimal parameter combinations based on health scores.

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow::array::{ArrayRef, Float64Array, UInt64Array};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
use sim_core::scenario::MatchingAlgorithmType;

use crate::health::{calculate_health_scores, HealthWeights};
use crate::metrics::SimulationResult;
use crate::parameters::ParameterSet;

/// Export simulation results to Parquet format.
///
/// Creates a Parquet file with columns for all metrics in `SimulationResult`.
///
/// # Arguments
///
/// * `results` - Vector of simulation results to export
/// * `path` - Path to output Parquet file
///
/// # Errors
///
/// Returns an error if file creation or Parquet writing fails.
pub fn export_to_parquet(
    results: &[SimulationResult],
    path: impl AsRef<Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    if results.is_empty() {
        return Err("No results to export".into());
    }

    // Build schema
    let schema = Schema::new(vec![
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
    ]);

    // Build arrays
    let total_riders: Vec<u64> = results.iter().map(|r| r.total_riders as u64).collect();
    let completed_riders: Vec<u64> = results.iter().map(|r| r.completed_riders as u64).collect();
    let abandoned_quote_riders: Vec<u64> = results
        .iter()
        .map(|r| r.abandoned_quote_riders as u64)
        .collect();
    let cancelled_riders: Vec<u64> = results.iter().map(|r| r.cancelled_riders as u64).collect();
    let conversion_rate: Vec<f64> = results.iter().map(|r| r.conversion_rate).collect();
    let platform_revenue: Vec<f64> = results.iter().map(|r| r.platform_revenue).collect();
    let driver_payouts: Vec<f64> = results.iter().map(|r| r.driver_payouts).collect();
    let total_fares_collected: Vec<f64> = results.iter().map(|r| r.total_fares_collected).collect();
    let avg_time_to_match_ms: Vec<f64> = results.iter().map(|r| r.avg_time_to_match_ms).collect();
    let median_time_to_match_ms: Vec<f64> =
        results.iter().map(|r| r.median_time_to_match_ms).collect();
    let p90_time_to_match_ms: Vec<f64> = results.iter().map(|r| r.p90_time_to_match_ms).collect();
    let avg_time_to_pickup_ms: Vec<f64> = results.iter().map(|r| r.avg_time_to_pickup_ms).collect();
    let median_time_to_pickup_ms: Vec<f64> =
        results.iter().map(|r| r.median_time_to_pickup_ms).collect();
    let p90_time_to_pickup_ms: Vec<f64> = results.iter().map(|r| r.p90_time_to_pickup_ms).collect();
    let completed_trips: Vec<u64> = results.iter().map(|r| r.completed_trips as u64).collect();
    let riders_abandoned_price: Vec<u64> = results
        .iter()
        .map(|r| r.riders_abandoned_price as u64)
        .collect();
    let riders_abandoned_eta: Vec<u64> = results
        .iter()
        .map(|r| r.riders_abandoned_eta as u64)
        .collect();
    let riders_abandoned_stochastic: Vec<u64> = results
        .iter()
        .map(|r| r.riders_abandoned_stochastic as u64)
        .collect();

    let arrays: Vec<ArrayRef> = vec![
        Arc::new(UInt64Array::from(total_riders)),
        Arc::new(UInt64Array::from(completed_riders)),
        Arc::new(UInt64Array::from(abandoned_quote_riders)),
        Arc::new(UInt64Array::from(cancelled_riders)),
        Arc::new(Float64Array::from(conversion_rate)),
        Arc::new(Float64Array::from(platform_revenue)),
        Arc::new(Float64Array::from(driver_payouts)),
        Arc::new(Float64Array::from(total_fares_collected)),
        Arc::new(Float64Array::from(avg_time_to_match_ms)),
        Arc::new(Float64Array::from(median_time_to_match_ms)),
        Arc::new(Float64Array::from(p90_time_to_match_ms)),
        Arc::new(Float64Array::from(avg_time_to_pickup_ms)),
        Arc::new(Float64Array::from(median_time_to_pickup_ms)),
        Arc::new(Float64Array::from(p90_time_to_pickup_ms)),
        Arc::new(UInt64Array::from(completed_trips)),
        Arc::new(UInt64Array::from(riders_abandoned_price)),
        Arc::new(UInt64Array::from(riders_abandoned_eta)),
        Arc::new(UInt64Array::from(riders_abandoned_stochastic)),
    ];

    let batch = RecordBatch::try_new(Arc::new(schema), arrays)?;

    let file = File::create(path)?;
    let props = WriterProperties::builder().build();
    let mut writer = ArrowWriter::try_new(file, batch.schema(), Some(props))?;
    writer.write(&batch)?;
    writer.close()?;

    Ok(())
}

/// Export simulation results to JSON format.
///
/// Creates a JSON file with an array of all results (serialized as JSON objects).
///
/// # Arguments
///
/// * `results` - Vector of simulation results to export
/// * `path` - Path to output JSON file
///
/// # Errors
///
/// Returns an error if file creation or JSON serialization fails.
pub fn export_to_json(
    results: &[SimulationResult],
    path: impl AsRef<Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(path)?;
    serde_json::to_writer_pretty(file, results)?;
    Ok(())
}

/// Export simulation results with parameters to CSV format.
///
/// Creates a CSV file with columns for all parameters and all metrics.
/// Parameters and results are paired by index (results[i] corresponds to parameter_sets[i]).
///
/// # Arguments
///
/// * `results` - Vector of simulation results to export
/// * `parameter_sets` - Vector of parameter sets (must match results in order)
/// * `path` - Path to output CSV file
///
/// # Errors
///
/// Returns an error if file creation or CSV writing fails, or if results and parameter_sets lengths don't match.
pub fn export_to_csv(
    results: &[SimulationResult],
    parameter_sets: &[ParameterSet],
    path: impl AsRef<Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    if results.is_empty() {
        return Err("No results to export".into());
    }
    if results.len() != parameter_sets.len() {
        return Err(format!(
            "Results length ({}) doesn't match parameter_sets length ({})",
            results.len(),
            parameter_sets.len()
        )
        .into());
    }

    let file = File::create(path)?;
    let mut wtr = csv::Writer::from_writer(file);

    // Write header
    wtr.write_record([
        // Parameter metadata
        "experiment_id",
        "run_id",
        "seed",
        // Pricing parameters
        "commission_rate",
        "base_fare",
        "per_km_rate",
        "surge_enabled",
        "surge_radius_k",
        "surge_max_multiplier",
        // Scenario parameters
        "num_riders",
        "num_drivers",
        "match_radius",
        "epoch_ms",
        "matching_algorithm_type",
        "batch_matching_enabled",
        "batch_interval_secs",
        "eta_weight",
        // Results
        "total_riders",
        "total_drivers",
        "completed_riders",
        "abandoned_quote_riders",
        "cancelled_riders",
        "conversion_rate",
        "platform_revenue",
        "driver_payouts",
        "total_fares_collected",
        "avg_time_to_match_ms",
        "median_time_to_match_ms",
        "p90_time_to_match_ms",
        "avg_time_to_pickup_ms",
        "median_time_to_pickup_ms",
        "p90_time_to_pickup_ms",
        "completed_trips",
        "riders_abandoned_price",
        "riders_abandoned_eta",
        "riders_abandoned_stochastic",
    ])?;

    // Write data rows
    for (result, param_set) in results.iter().zip(parameter_sets.iter()) {
        let pricing = param_set.params.pricing_config.as_ref();
        let matching_alg = param_set.params.matching_algorithm_type.as_ref();
        let matching_alg_str = match matching_alg {
            Some(MatchingAlgorithmType::Simple) => "Simple",
            Some(MatchingAlgorithmType::CostBased) => "CostBased",
            Some(MatchingAlgorithmType::Hungarian) => "Hungarian",
            None => "",
        };

        wtr.write_record([
            // Parameter metadata
            &param_set.experiment_id,
            &param_set.run_id.to_string(),
            &param_set.seed.to_string(),
            // Pricing parameters
            &pricing
                .map(|p| p.commission_rate.to_string())
                .unwrap_or_default(),
            &pricing.map(|p| p.base_fare.to_string()).unwrap_or_default(),
            &pricing
                .map(|p| p.per_km_rate.to_string())
                .unwrap_or_default(),
            &pricing
                .map(|p| p.surge_enabled.to_string())
                .unwrap_or_default(),
            &pricing
                .map(|p| p.surge_radius_k.to_string())
                .unwrap_or_default(),
            &pricing
                .map(|p| p.surge_max_multiplier.to_string())
                .unwrap_or_default(),
            // Scenario parameters
            &param_set.params.num_riders.to_string(),
            &param_set.params.num_drivers.to_string(),
            &param_set.params.match_radius.to_string(),
            &param_set
                .params
                .epoch_ms
                .map(|e| e.to_string())
                .unwrap_or_default(),
            matching_alg_str,
            &param_set
                .params
                .batch_matching_enabled
                .map(|b| b.to_string())
                .unwrap_or_default(),
            &param_set
                .params
                .batch_interval_secs
                .map(|i| i.to_string())
                .unwrap_or_default(),
            &param_set
                .params
                .eta_weight
                .map(|w| w.to_string())
                .unwrap_or_default(),
            // Results
            &result.total_riders.to_string(),
            &result.total_drivers.to_string(),
            &result.completed_riders.to_string(),
            &result.abandoned_quote_riders.to_string(),
            &result.cancelled_riders.to_string(),
            &result.conversion_rate.to_string(),
            &result.platform_revenue.to_string(),
            &result.driver_payouts.to_string(),
            &result.total_fares_collected.to_string(),
            &result.avg_time_to_match_ms.to_string(),
            &result.median_time_to_match_ms.to_string(),
            &result.p90_time_to_match_ms.to_string(),
            &result.avg_time_to_pickup_ms.to_string(),
            &result.median_time_to_pickup_ms.to_string(),
            &result.p90_time_to_pickup_ms.to_string(),
            &result.completed_trips.to_string(),
            &result.riders_abandoned_price.to_string(),
            &result.riders_abandoned_eta.to_string(),
            &result.riders_abandoned_stochastic.to_string(),
        ])?;
    }

    wtr.flush()?;
    Ok(())
}

/// Find the parameter set with the highest health score.
///
/// Calculates health scores for all results and returns the parameter set
/// corresponding to the result with the highest score.
///
/// # Arguments
///
/// * `results` - Vector of simulation results
/// * `parameter_sets` - Vector of parameter sets (must match results in order)
/// * `weights` - Health weights for score calculation
///
/// # Returns
///
/// Option containing the best parameter set, or None if inputs are empty or mismatched.
pub fn find_best_parameters<'a>(
    results: &'a [SimulationResult],
    parameter_sets: &'a [ParameterSet],
    weights: &'a HealthWeights,
) -> Option<&'a ParameterSet> {
    if results.is_empty() || results.len() != parameter_sets.len() {
        return None;
    }

    let scores = calculate_health_scores(results, weights);
    let (best_idx, _best_score) = scores
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap();

    Some(&parameter_sets[best_idx])
}

/// Find the best result index (convenience function when parameter sets aren't available).
///
/// This is a convenience function that finds the best result by health score.
/// For full functionality with parameter sets, use `find_best_parameters`.
///
/// # Arguments
///
/// * `results` - Vector of simulation results
/// * `weights` - Health weights for score calculation
///
/// # Returns
///
/// Index of the best result, or None if results are empty.
pub fn find_best_result_index(
    results: &[SimulationResult],
    weights: &HealthWeights,
) -> Option<usize> {
    if results.is_empty() {
        return None;
    }

    let scores = calculate_health_scores(results, weights);
    let (best_idx, _best_score) = scores
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap();

    Some(best_idx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::SimulationResult;
    use tempfile::NamedTempFile;

    #[test]
    fn test_export_to_json() {
        let results = vec![SimulationResult {
            total_riders: 100,
            total_drivers: 20,
            completed_riders: 80,
            abandoned_quote_riders: 10,
            cancelled_riders: 10,
            conversion_rate: 0.8,
            platform_revenue: 1000.0,
            driver_payouts: 5000.0,
            total_fares_collected: 6000.0,
            avg_time_to_match_ms: 1000.0,
            median_time_to_match_ms: 1000.0,
            p90_time_to_match_ms: 2000.0,
            avg_time_to_pickup_ms: 5000.0,
            median_time_to_pickup_ms: 5000.0,
            p90_time_to_pickup_ms: 10000.0,
            completed_trips: 80,
            riders_abandoned_price: 5,
            riders_abandoned_eta: 3,
            riders_abandoned_stochastic: 2,
        }];

        let file = NamedTempFile::new().unwrap();
        export_to_json(&results, file.path()).unwrap();

        // Verify file was created and contains JSON
        let contents = std::fs::read_to_string(file.path()).unwrap();
        assert!(contents.contains("conversion_rate"));
    }

    #[test]
    fn test_find_best_result_index() {
        let results = vec![
            SimulationResult {
                total_riders: 100,
                total_drivers: 15,
                completed_riders: 60,
                abandoned_quote_riders: 30,
                cancelled_riders: 10,
                conversion_rate: 0.6,
                platform_revenue: 500.0,
                driver_payouts: 2500.0,
                total_fares_collected: 3000.0,
                avg_time_to_match_ms: 2000.0,
                median_time_to_match_ms: 2000.0,
                p90_time_to_match_ms: 4000.0,
                avg_time_to_pickup_ms: 10000.0,
                median_time_to_pickup_ms: 10000.0,
                p90_time_to_pickup_ms: 20000.0,
                completed_trips: 60,
                riders_abandoned_price: 15,
                riders_abandoned_eta: 10,
                riders_abandoned_stochastic: 5,
            },
            SimulationResult {
                total_riders: 100,
                total_drivers: 20,
                completed_riders: 80,
                abandoned_quote_riders: 10,
                cancelled_riders: 10,
                conversion_rate: 0.8,
                platform_revenue: 1000.0,
                driver_payouts: 5000.0,
                total_fares_collected: 6000.0,
                avg_time_to_match_ms: 1000.0,
                median_time_to_match_ms: 1000.0,
                p90_time_to_match_ms: 2000.0,
                avg_time_to_pickup_ms: 5000.0,
                median_time_to_pickup_ms: 5000.0,
                p90_time_to_pickup_ms: 10000.0,
                completed_trips: 80,
                riders_abandoned_price: 5,
                riders_abandoned_eta: 3,
                riders_abandoned_stochastic: 2,
            },
        ];

        let weights = HealthWeights::default();
        let best_idx = find_best_result_index(&results, &weights).unwrap();
        assert_eq!(best_idx, 1); // Second result should be better
    }
}
