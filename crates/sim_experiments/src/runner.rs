//! Parallel simulation execution using rayon.
//!
//! This module provides functions to run single simulations and execute
//! multiple simulations in parallel for parameter sweeps.

use bevy_ecs::prelude::World;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use sim_core::runner::{initialize_simulation, run_until_empty, simulation_schedule};
use sim_core::scenario::build_scenario;
use sim_core::telemetry::{SimSnapshots, SimTelemetry};
use sim_core::telemetry_export::{write_snapshot_counts_parquet, write_trips_parquet};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::metrics::{extract_metrics, SimulationResult};
use crate::parameters::ParameterSet;

#[derive(Debug, Clone)]
pub struct SimulationArtifacts {
    pub metrics: SimulationResult,
    pub trip_data_parquet: Vec<u8>,
    pub snapshot_counts_parquet: Vec<u8>,
}

/// Shared simulation primitive used by local sweeps and serverless workers.
///
/// Runs one parameter set to completion and returns metrics plus exported
/// telemetry parquet payloads.
pub fn run_single_simulation_with_artifacts(
    param_set: &ParameterSet,
) -> Result<SimulationArtifacts, String> {
    let mut world = World::new();
    let mut params = param_set.scenario_params();

    if params.simulation_end_time_ms.is_none() {
        let request_window_ms = params.request_window_ms;
        let end_time_ms = request_window_ms.saturating_add(2 * 60 * 60 * 1000);
        params.simulation_end_time_ms = Some(end_time_ms);
    }

    build_scenario(&mut world, params);
    initialize_simulation(&mut world);

    let mut schedule = simulation_schedule();
    let _steps = run_until_empty(&mut world, &mut schedule, 2_000_000);

    let metrics = extract_metrics(&mut world);
    world
        .get_resource::<SimTelemetry>()
        .ok_or_else(|| "SimTelemetry resource not found".to_string())?;
    let snapshots = world
        .get_resource::<SimSnapshots>()
        .ok_or_else(|| "SimSnapshots resource not found".to_string())?;

    let trip_data_parquet = serialize_to_parquet_bytes(
        |path| write_trips_parquet(path, snapshots),
        &param_set.experiment_id,
        param_set.run_id,
        "trip-data",
    )?;
    let snapshot_counts_parquet = serialize_to_parquet_bytes(
        |path| write_snapshot_counts_parquet(path, snapshots),
        &param_set.experiment_id,
        param_set.run_id,
        "snapshot-counts",
    )?;

    Ok(SimulationArtifacts {
        metrics,
        trip_data_parquet,
        snapshot_counts_parquet,
    })
}

/// Run a single simulation with the given parameter set.
///
/// Creates a new world, builds the scenario, runs the simulation to completion,
/// and extracts metrics from the results.
///
/// # Arguments
///
/// * `param_set` - Parameter configuration for this simulation run
///
/// # Returns
///
/// A `SimulationResult` containing all extracted metrics.
pub fn run_single_simulation(param_set: &ParameterSet) -> SimulationResult {
    run_single_simulation_with_artifacts(param_set)
        .expect("single simulation should execute and export telemetry")
        .metrics
}

fn serialize_to_parquet_bytes<F>(
    write_fn: F,
    id: &str,
    index: usize,
    suffix: &str,
) -> Result<Vec<u8>, String>
where
    F: FnOnce(&std::path::Path) -> Result<(), Box<dyn std::error::Error>>,
{
    let mut temp_path = std::env::temp_dir();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("Failed to read clock for parquet export: {error}"))?
        .as_nanos();
    temp_path.push(format!(
        "sim-experiment-{id}-{index}-{suffix}-{timestamp}.parquet"
    ));

    write_fn(&temp_path).map_err(|error| format!("Parquet export failed: {error}"))?;
    let bytes = fs::read(&temp_path)
        .map_err(|error| format!("Failed to read exported parquet file: {error}"))?;
    let _ = fs::remove_file(&temp_path);
    Ok(bytes)
}

/// Run multiple simulations in parallel.
///
/// Uses rayon to execute simulations concurrently across available CPU cores.
/// Each simulation runs independently with no shared state.
///
/// # Arguments
///
/// * `parameter_sets` - Vector of parameter sets to run
/// * `num_threads` - Optional number of threads to use. If None, uses rayon's default.
///
/// # Returns
///
/// Vector of `SimulationResult` in the same order as input parameter sets.
pub fn run_parallel_experiments(
    parameter_sets: Vec<ParameterSet>,
    num_threads: Option<usize>,
) -> Vec<SimulationResult> {
    run_parallel_experiments_with_progress(parameter_sets, num_threads, true)
}

/// Run multiple simulations in parallel with optional progress bar.
///
/// Uses rayon to execute simulations concurrently across available CPU cores.
/// Each simulation runs independently with no shared state.
///
/// # Arguments
///
/// * `parameter_sets` - Vector of parameter sets to run
/// * `num_threads` - Optional number of threads to use. If None, uses rayon's default.
/// * `show_progress` - Whether to display a progress bar
///
/// # Returns
///
/// Vector of `SimulationResult` in the same order as input parameter sets.
pub fn run_parallel_experiments_with_progress(
    parameter_sets: Vec<ParameterSet>,
    num_threads: Option<usize>,
    show_progress: bool,
) -> Vec<SimulationResult> {
    let total = parameter_sets.len();
    let pb = if show_progress && total > 0 {
        let bar = ProgressBar::new(total as u64);
        bar.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
                )
                .unwrap()
                .progress_chars("#>-"),
        );
        Some(bar)
    } else {
        None
    };

    let pool = if let Some(threads) = num_threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("Failed to create thread pool")
    } else {
        rayon::ThreadPoolBuilder::new()
            .build()
            .expect("Failed to create thread pool")
    };

    let pb_clone = pb.clone();
    let results = pool.install(|| {
        parameter_sets
            .par_iter()
            .map(|param_set| {
                let result = run_single_simulation(param_set);
                if let Some(ref progress_bar) = pb_clone {
                    progress_bar.inc(1);
                }
                result
            })
            .collect()
    });

    if let Some(ref progress_bar) = pb {
        progress_bar.finish_with_message("Completed");
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parameters::ParameterSpace;

    #[test]
    fn test_single_simulation() {
        let space = ParameterSpace::grid()
            .num_riders(vec![10])
            .num_drivers(vec![3]);
        let sets = space.generate();
        let result = run_single_simulation(&sets[0]);

        // Basic sanity checks
        assert!(result.total_riders > 0);
        assert!(result.total_drivers > 0);
    }

    #[test]
    fn test_parallel_experiments() {
        let space = ParameterSpace::grid()
            .num_riders(vec![10, 20])
            .num_drivers(vec![3, 5]);
        let sets = space.generate();
        let results = run_parallel_experiments_with_progress(sets, Some(2), false);

        assert_eq!(results.len(), 4); // 2 * 2 = 4 combinations
        for result in &results {
            assert!(result.total_riders > 0);
        }
    }
}
