//! Parameter variation framework for exploring simulation parameter space.
//!
//! This module provides tools for defining parameter spaces and generating
//! parameter sets for parallel experimentation. Supports grid search and
//! random sampling strategies.

use sim_core::scenario::{MatchingAlgorithmType, ScenarioParams};
use sim_core::traffic::TrafficProfileKind;

mod combinations;
mod constraints;
mod conversion;
mod sampling;

#[cfg(test)]
mod tests;

/// A single parameter configuration for a simulation run.
///
/// Wraps `ScenarioParams` with additional experiment metadata for tracking
/// and reproducibility.
#[derive(Debug, Clone)]
pub struct ParameterSet {
    /// Base scenario parameters.
    pub params: ScenarioParams,
    /// Unique experiment ID for this parameter configuration.
    pub experiment_id: String,
    /// Run ID within the experiment (for multiple runs with same params).
    pub run_id: usize,
    /// Seed used for this run (ensures reproducibility).
    pub seed: u64,
}

impl ParameterSet {
    /// Create a new parameter set with the given parameters and metadata.
    pub fn new(params: ScenarioParams, experiment_id: String, run_id: usize, seed: u64) -> Self {
        Self {
            params,
            experiment_id,
            run_id,
            seed,
        }
    }

    /// Get the scenario params with seed applied.
    pub fn scenario_params(&self) -> ScenarioParams {
        let mut params = self.params.clone();
        params.seed = Some(self.seed);
        params
    }
}

/// Defines a parameter space for exploration.
///
/// Supports grid search (Cartesian product) and random sampling strategies.
#[derive(Debug, Clone)]
pub struct ParameterSpace {
    /// Base parameters (used as defaults for unspecified parameters).
    pub(super) base: ScenarioParams,
    /// Commission rates to explore.
    pub(super) commission_rates: Vec<f64>,
    /// Base fares to explore.
    pub(super) base_fares: Vec<f64>,
    /// Per-km rates to explore.
    pub(super) per_km_rates: Vec<f64>,
    /// Surge enabled values to explore.
    pub(super) surge_enabled: Vec<bool>,
    /// Surge radius k values to explore.
    pub(super) surge_radius_k: Vec<u32>,
    /// Surge max multipliers to explore.
    pub(super) surge_max_multipliers: Vec<f64>,
    /// Number of riders to explore.
    pub(super) num_riders: Vec<usize>,
    /// Number of drivers to explore.
    pub(super) num_drivers: Vec<usize>,
    /// Match radii to explore.
    pub(super) match_radii: Vec<u32>,
    /// Epoch values (start datetime in ms) to explore.
    pub(super) epoch_ms: Vec<Option<i64>>,
    /// Simulation duration in hours to explore.
    pub(super) simulation_duration_hours: Vec<Option<u64>>,
    /// Matching algorithm types to explore.
    pub(super) matching_algorithm_types: Vec<MatchingAlgorithmType>,
    /// Batch matching enabled values to explore.
    pub(super) batch_matching_enabled: Vec<bool>,
    /// Batch interval (seconds) values to explore.
    pub(super) batch_interval_secs: Vec<u64>,
    /// ETA weights to explore.
    pub(super) eta_weights: Vec<f64>,
    /// Traffic profiles to explore.
    pub(super) traffic_profiles: Vec<TrafficProfileKind>,
    /// Dynamic congestion enabled values to explore.
    pub(super) dynamic_congestion_enabled: Vec<bool>,
    /// Base speed (km/h) values to explore.
    pub(super) base_speed_kmh: Vec<Option<f64>>,
}

impl ParameterSpace {
    /// Create a new parameter space with default base parameters.
    pub fn new() -> Self {
        Self {
            base: ScenarioParams::default(),
            commission_rates: vec![],
            base_fares: vec![],
            per_km_rates: vec![],
            surge_enabled: vec![],
            surge_radius_k: vec![],
            surge_max_multipliers: vec![],
            num_riders: vec![],
            num_drivers: vec![],
            match_radii: vec![],
            epoch_ms: vec![],
            simulation_duration_hours: vec![],
            matching_algorithm_types: vec![],
            batch_matching_enabled: vec![],
            batch_interval_secs: vec![],
            eta_weights: vec![],
            traffic_profiles: vec![],
            dynamic_congestion_enabled: vec![],
            base_speed_kmh: vec![],
        }
    }

    /// Create a new parameter space for grid search.
    pub fn grid() -> Self {
        Self::new()
    }

    /// Set commission rates to explore.
    pub fn commission_rate(mut self, rates: Vec<f64>) -> Self {
        self.commission_rates = rates;
        self
    }

    /// Set base fares to explore.
    pub fn base_fare(mut self, fares: Vec<f64>) -> Self {
        self.base_fares = fares;
        self
    }

    /// Set per-km rates to explore.
    pub fn per_km_rate(mut self, rates: Vec<f64>) -> Self {
        self.per_km_rates = rates;
        self
    }

    /// Set surge enabled values to explore.
    pub fn surge_enabled(mut self, enabled: Vec<bool>) -> Self {
        self.surge_enabled = enabled;
        self
    }

    /// Set surge radius k values to explore.
    pub fn surge_radius_k(mut self, radius_k: Vec<u32>) -> Self {
        self.surge_radius_k = radius_k;
        self
    }

    /// Set surge max multipliers to explore.
    pub fn surge_max_multiplier(mut self, multipliers: Vec<f64>) -> Self {
        self.surge_max_multipliers = multipliers;
        self
    }

    /// Set number of riders to explore.
    pub fn num_riders(mut self, counts: Vec<usize>) -> Self {
        self.num_riders = counts;
        self
    }

    /// Set number of drivers to explore.
    pub fn num_drivers(mut self, counts: Vec<usize>) -> Self {
        self.num_drivers = counts;
        self
    }

    /// Set match radii to explore.
    pub fn match_radius(mut self, radii: Vec<u32>) -> Self {
        self.match_radii = radii;
        self
    }

    /// Set epoch (start datetime) values to explore.
    pub fn epoch_ms(mut self, epochs: Vec<Option<i64>>) -> Self {
        self.epoch_ms = epochs;
        self
    }

    /// Set simulation duration (in hours) values to explore.
    pub fn simulation_duration_hours(mut self, durations: Vec<Option<u64>>) -> Self {
        self.simulation_duration_hours = durations;
        self
    }

    /// Set matching algorithm types to explore.
    pub fn matching_algorithm_type(mut self, types: Vec<MatchingAlgorithmType>) -> Self {
        self.matching_algorithm_types = types;
        self
    }

    /// Set batch matching enabled values to explore.
    pub fn batch_matching_enabled(mut self, enabled: Vec<bool>) -> Self {
        self.batch_matching_enabled = enabled;
        self
    }

    /// Set batch interval (seconds) values to explore.
    pub fn batch_interval_secs(mut self, intervals: Vec<u64>) -> Self {
        self.batch_interval_secs = intervals;
        self
    }

    /// Set ETA weights to explore.
    pub fn eta_weight(mut self, weights: Vec<f64>) -> Self {
        self.eta_weights = weights;
        self
    }

    /// Set traffic profiles to explore.
    pub fn traffic_profile(mut self, profiles: Vec<TrafficProfileKind>) -> Self {
        self.traffic_profiles = profiles;
        self
    }

    /// Set dynamic congestion enabled values to explore.
    pub fn dynamic_congestion_enabled(mut self, enabled: Vec<bool>) -> Self {
        self.dynamic_congestion_enabled = enabled;
        self
    }

    /// Set base speed (km/h) values to explore.
    pub fn base_speed_kmh(mut self, speeds: Vec<Option<f64>>) -> Self {
        self.base_speed_kmh = speeds;
        self
    }

    /// Set base parameters (used as defaults).
    pub fn with_base(mut self, base: ScenarioParams) -> Self {
        self.base = base;
        self
    }

    /// Generate all parameter sets using grid search (Cartesian product).
    ///
    /// Each combination of specified parameters will be generated.
    /// Parameters not specified will use values from the base configuration.
    /// Invalid combinations (e.g., Hungarian matching without batch matching) are filtered out.
    pub fn generate(&self) -> Vec<ParameterSet> {
        let variations = combinations::ParameterVariations::from_space(self);

        variations
            .generate_combinations()
            .into_iter()
            .filter(constraints::is_valid_combination)
            .enumerate()
            .map(|(experiment_id, combo)| {
                conversion::combination_to_parameter_set(&self.base, combo, experiment_id)
            })
            .collect()
    }
}

impl Default for ParameterSpace {
    fn default() -> Self {
        Self::new()
    }
}
