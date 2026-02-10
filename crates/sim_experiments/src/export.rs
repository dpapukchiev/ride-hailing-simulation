//! Result export and analysis utilities.
//!
//! This module provides functions to export experiment results to Parquet and JSON,
//! and to find optimal parameter combinations based on health scores.

use std::path::Path;

use crate::health::HealthWeights;
use crate::metrics::SimulationResult;
use crate::parameters::ParameterSet;

#[path = "export/csv.rs"]
mod csv;
#[path = "export/json.rs"]
mod json;
#[path = "export/parquet.rs"]
mod parquet;
#[path = "export/ranking.rs"]
mod ranking;
#[path = "export/writer_utils.rs"]
mod writer_utils;

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
    writer_utils::ensure_not_empty(results)?;
    let file = writer_utils::create_output_file(path)?;
    parquet::export_to_parquet_impl(results, file)
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
    let file = writer_utils::create_output_file(path)?;
    json::export_to_json_impl(results, file)
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
    writer_utils::ensure_not_empty(results)?;
    let file = writer_utils::create_output_file(path)?;
    csv::export_to_csv_impl(results, parameter_sets, file)
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
    ranking::find_best_parameters_impl(results, parameter_sets, weights)
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
    ranking::find_best_index_by_health(results, weights)
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
        assert_eq!(best_idx, 1);
    }
}
