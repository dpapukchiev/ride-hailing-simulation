//! Parallel simulation execution using rayon.
//!
//! This module provides functions to run single simulations and execute
//! multiple simulations in parallel for parameter sweeps.

use bevy_ecs::prelude::World;
use rayon::prelude::*;
use sim_core::runner::{initialize_simulation, run_until_empty, simulation_schedule};
use sim_core::scenario::build_scenario;
use indicatif::{ProgressBar, ProgressStyle};

use crate::metrics::{extract_metrics, SimulationResult};
use crate::parameters::ParameterSet;

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
    let mut world = World::new();
    let mut params = param_set.scenario_params();
    
    // Set simulation end time if not already set (fallback to prevent infinite loops)
    // If simulation_duration_hours was set in ParameterSpace, it should already be in params
    if params.simulation_end_time_ms.is_none() {
        let request_window_ms = params.request_window_ms;
        // Default: request window + 2 hours buffer for trips to complete
        let end_time_ms = request_window_ms.saturating_add(2 * 60 * 60 * 1000);
        params.simulation_end_time_ms = Some(end_time_ms);
    }
    
    build_scenario(&mut world, params);
    initialize_simulation(&mut world);
    
    let mut schedule = simulation_schedule();
    // Allow enough steps for large simulations (2M should be sufficient)
    let _steps = run_until_empty(&mut world, &mut schedule, 2_000_000);
    
    extract_metrics(&mut world)
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
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
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

    let pb_clone = pb.as_ref().map(|bar| bar.clone());
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
