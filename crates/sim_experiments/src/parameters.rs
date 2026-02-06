//! Parameter variation framework for exploring simulation parameter space.
//!
//! This module provides tools for defining parameter spaces and generating
//! parameter sets for parallel experimentation. Supports grid search and
//! random sampling strategies.

use sim_core::pricing::PricingConfig;
use sim_core::scenario::ScenarioParams;
use std::collections::HashSet;

/// Represents a single parameter combination.
#[derive(Debug, Clone)]
struct ParameterCombination {
    commission_rate: f64,
    base_fare: f64,
    per_km_rate: f64,
    surge_enabled: bool,
    surge_max_multiplier: f64,
    num_riders: usize,
    num_drivers: usize,
    match_radius: u32,
    epoch_ms: Option<i64>,
    simulation_duration_hours: Option<u64>,
}

/// Holds all parameter variations to explore.
struct ParameterVariations {
    commission_rates: Vec<f64>,
    base_fares: Vec<f64>,
    per_km_rates: Vec<f64>,
    surge_enabled: Vec<bool>,
    surge_max_multipliers: Vec<f64>,
    num_riders: Vec<usize>,
    num_drivers: Vec<usize>,
    match_radii: Vec<u32>,
    epoch_ms_values: Vec<Option<i64>>,
    simulation_duration_hours: Vec<Option<u64>>,
}

impl ParameterVariations {
    fn from_space(space: &ParameterSpace) -> Self {
        let default_pricing = space.base.pricing_config.as_ref();
        
        Self {
            commission_rates: if space.commission_rates.is_empty() {
                vec![default_pricing.map(|p| p.commission_rate).unwrap_or(0.0)]
            } else {
                space.commission_rates.clone()
            },
            base_fares: if space.base_fares.is_empty() {
                vec![default_pricing.map(|p| p.base_fare).unwrap_or(2.50)]
            } else {
                space.base_fares.clone()
            },
            per_km_rates: if space.per_km_rates.is_empty() {
                vec![default_pricing.map(|p| p.per_km_rate).unwrap_or(1.50)]
            } else {
                space.per_km_rates.clone()
            },
            surge_enabled: if space.surge_enabled.is_empty() {
                vec![default_pricing.map(|p| p.surge_enabled).unwrap_or(false)]
            } else {
                space.surge_enabled.clone()
            },
            surge_max_multipliers: if space.surge_max_multipliers.is_empty() {
                vec![default_pricing.map(|p| p.surge_max_multiplier).unwrap_or(2.0)]
            } else {
                space.surge_max_multipliers.clone()
            },
            num_riders: if space.num_riders.is_empty() {
                vec![space.base.num_riders]
            } else {
                space.num_riders.clone()
            },
            num_drivers: if space.num_drivers.is_empty() {
                vec![space.base.num_drivers]
            } else {
                space.num_drivers.clone()
            },
            match_radii: if space.match_radii.is_empty() {
                vec![space.base.match_radius]
            } else {
                space.match_radii.clone()
            },
            epoch_ms_values: if space.epoch_ms.is_empty() {
                vec![space.base.epoch_ms]
            } else {
                space.epoch_ms.clone()
            },
            simulation_duration_hours: if space.simulation_duration_hours.is_empty() {
                vec![None] // Default: no end time (will be set automatically in runner)
            } else {
                space.simulation_duration_hours.clone()
            },
        }
    }

    /// Generate all combinations using Cartesian product.
    fn generate_combinations(&self) -> impl Iterator<Item = ParameterCombination> + '_ {
        // Build combinations by iteratively expanding partial combinations
        // This avoids deep nesting by using helper functions
        self.commission_rates.iter()
            .flat_map(move |&commission_rate| {
                self.expand_with_base_fares(commission_rate)
            })
    }

    /// Helper to expand combinations with base_fares and subsequent parameters.
    fn expand_with_base_fares(&self, commission_rate: f64) -> impl Iterator<Item = ParameterCombination> + '_ {
        self.base_fares.iter()
            .flat_map(move |&base_fare| {
                self.expand_with_per_km_rates(commission_rate, base_fare)
            })
    }

    /// Helper to expand combinations with per_km_rates and subsequent parameters.
    fn expand_with_per_km_rates(&self, commission_rate: f64, base_fare: f64) -> impl Iterator<Item = ParameterCombination> + '_ {
        self.per_km_rates.iter()
            .flat_map(move |&per_km_rate| {
                self.expand_with_surge_enabled(commission_rate, base_fare, per_km_rate)
            })
    }

    /// Helper to expand combinations with surge_enabled and subsequent parameters.
    fn expand_with_surge_enabled(&self, commission_rate: f64, base_fare: f64, per_km_rate: f64) -> impl Iterator<Item = ParameterCombination> + '_ {
        self.surge_enabled.iter()
            .flat_map(move |&surge_enabled| {
                self.expand_with_surge_multipliers(commission_rate, base_fare, per_km_rate, surge_enabled)
            })
    }

    /// Helper to expand combinations with surge_max_multipliers and subsequent parameters.
    fn expand_with_surge_multipliers(&self, commission_rate: f64, base_fare: f64, per_km_rate: f64, surge_enabled: bool) -> impl Iterator<Item = ParameterCombination> + '_ {
        self.surge_max_multipliers.iter()
            .flat_map(move |&surge_max_multiplier| {
                self.expand_with_num_riders(commission_rate, base_fare, per_km_rate, surge_enabled, surge_max_multiplier)
            })
    }

    /// Helper to expand combinations with num_riders and subsequent parameters.
    fn expand_with_num_riders(&self, commission_rate: f64, base_fare: f64, per_km_rate: f64, surge_enabled: bool, surge_max_multiplier: f64) -> impl Iterator<Item = ParameterCombination> + '_ {
        self.num_riders.iter()
            .flat_map(move |&num_riders| {
                self.expand_with_num_drivers(commission_rate, base_fare, per_km_rate, surge_enabled, surge_max_multiplier, num_riders)
            })
    }

    /// Helper to expand combinations with num_drivers and subsequent parameters.
    fn expand_with_num_drivers(&self, commission_rate: f64, base_fare: f64, per_km_rate: f64, surge_enabled: bool, surge_max_multiplier: f64, num_riders: usize) -> impl Iterator<Item = ParameterCombination> + '_ {
        self.num_drivers.iter()
            .flat_map(move |&num_drivers| {
                self.expand_with_match_radii(commission_rate, base_fare, per_km_rate, surge_enabled, surge_max_multiplier, num_riders, num_drivers)
            })
    }

    /// Helper to expand combinations with match_radii and subsequent parameters.
    fn expand_with_match_radii(&self, commission_rate: f64, base_fare: f64, per_km_rate: f64, surge_enabled: bool, surge_max_multiplier: f64, num_riders: usize, num_drivers: usize) -> impl Iterator<Item = ParameterCombination> + '_ {
        self.match_radii.iter()
            .flat_map(move |&match_radius| {
                self.expand_with_epoch_ms(commission_rate, base_fare, per_km_rate, surge_enabled, surge_max_multiplier, num_riders, num_drivers, match_radius)
            })
    }

    /// Helper to expand combinations with epoch_ms and subsequent parameters.
    fn expand_with_epoch_ms(&self, commission_rate: f64, base_fare: f64, per_km_rate: f64, surge_enabled: bool, surge_max_multiplier: f64, num_riders: usize, num_drivers: usize, match_radius: u32) -> impl Iterator<Item = ParameterCombination> + '_ {
        self.epoch_ms_values.iter()
            .flat_map(move |&epoch_ms| {
                self.expand_with_duration(commission_rate, base_fare, per_km_rate, surge_enabled, surge_max_multiplier, num_riders, num_drivers, match_radius, epoch_ms)
            })
    }

    /// Helper to expand combinations with simulation_duration_hours (final step).
    fn expand_with_duration(&self, commission_rate: f64, base_fare: f64, per_km_rate: f64, surge_enabled: bool, surge_max_multiplier: f64, num_riders: usize, num_drivers: usize, match_radius: u32, epoch_ms: Option<i64>) -> impl Iterator<Item = ParameterCombination> + '_ {
        self.simulation_duration_hours.iter()
            .map(move |&simulation_duration_hours| {
                ParameterCombination {
                    commission_rate,
                    base_fare,
                    per_km_rate,
                    surge_enabled,
                    surge_max_multiplier,
                    num_riders,
                    num_drivers,
                    match_radius,
                    epoch_ms,
                    simulation_duration_hours,
                }
            })
    }
}

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
    pub fn new(
        params: ScenarioParams,
        experiment_id: String,
        run_id: usize,
        seed: u64,
    ) -> Self {
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
    base: ScenarioParams,
    /// Commission rates to explore.
    commission_rates: Vec<f64>,
    /// Base fares to explore.
    base_fares: Vec<f64>,
    /// Per-km rates to explore.
    per_km_rates: Vec<f64>,
    /// Surge enabled values to explore.
    surge_enabled: Vec<bool>,
    /// Surge max multipliers to explore.
    surge_max_multipliers: Vec<f64>,
    /// Number of riders to explore.
    num_riders: Vec<usize>,
    /// Number of drivers to explore.
    num_drivers: Vec<usize>,
    /// Match radii to explore.
    match_radii: Vec<u32>,
    /// Epoch values (start datetime in ms) to explore.
    epoch_ms: Vec<Option<i64>>,
    /// Simulation duration in hours to explore.
    simulation_duration_hours: Vec<Option<u64>>,
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
            surge_max_multipliers: vec![],
            num_riders: vec![],
            num_drivers: vec![],
            match_radii: vec![],
            epoch_ms: vec![],
            simulation_duration_hours: vec![],
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

    /// Set base parameters (used as defaults).
    pub fn with_base(mut self, base: ScenarioParams) -> Self {
        self.base = base;
        self
    }

    /// Generate all parameter sets using grid search (Cartesian product).
    ///
    /// Each combination of specified parameters will be generated.
    /// Parameters not specified will use values from the base configuration.
    pub fn generate(&self) -> Vec<ParameterSet> {
        // Collect all parameter variations with defaults
        let variations = ParameterVariations::from_space(self);
        
        // Generate Cartesian product using iterator approach
        variations.generate_combinations()
            .enumerate()
            .map(|(experiment_id, combo)| {
                let mut params = self.base.clone();
                params.num_riders = combo.num_riders;
                params.num_drivers = combo.num_drivers;
                params.match_radius = combo.match_radius;
                params.epoch_ms = combo.epoch_ms;

                // Set simulation end time from duration
                if let Some(duration_hours) = combo.simulation_duration_hours {
                    let request_window_ms = params.request_window_ms;
                    // End time = request window + duration (with buffer for trips to complete)
                    let end_time_ms = request_window_ms.saturating_add(duration_hours * 60 * 60 * 1000);
                    params.simulation_end_time_ms = Some(end_time_ms);
                }

                let pricing_config = PricingConfig {
                    base_fare: combo.base_fare,
                    per_km_rate: combo.per_km_rate,
                    commission_rate: combo.commission_rate,
                    surge_enabled: combo.surge_enabled,
                    surge_radius_k: self.base.pricing_config.as_ref().map(|p| p.surge_radius_k).unwrap_or(1),
                    surge_max_multiplier: combo.surge_max_multiplier,
                };
                params.pricing_config = Some(pricing_config);

                let seed = (experiment_id as u64).wrapping_mul(0x9e3779b9);

                ParameterSet::new(
                    params,
                    format!("exp_{}", experiment_id),
                    0,
                    seed,
                )
            })
            .collect()
    }

    /// Generate random parameter sets (Monte Carlo sampling).
    ///
    /// Samples `count` parameter sets randomly from the defined space.
    /// Requires at least one parameter to have multiple values.
    /// If duplicates are encountered, continues sampling until `count` unique sets are generated.
    pub fn sample_random(&self, count: usize, seed: u64) -> Vec<ParameterSet> {
        use rand::Rng;
        use rand::SeedableRng;
        use rand::rngs::StdRng;

        let mut rng = StdRng::seed_from_u64(seed);
        let mut parameter_sets = Vec::new();
        let mut seen = HashSet::new();
        let mut attempts = 0;
        const MAX_ATTEMPTS: usize = 10000; // Prevent infinite loops

        while parameter_sets.len() < count && attempts < MAX_ATTEMPTS {
            attempts += 1;
            let mut params = self.base.clone();

            // Sample pricing parameters
            let commission_rate = if !self.commission_rates.is_empty() {
                self.commission_rates[rng.gen_range(0..self.commission_rates.len())]
            } else {
                self.base.pricing_config.as_ref().map(|p| p.commission_rate).unwrap_or(0.0)
            };

            let base_fare = if !self.base_fares.is_empty() {
                self.base_fares[rng.gen_range(0..self.base_fares.len())]
            } else {
                self.base.pricing_config.as_ref().map(|p| p.base_fare).unwrap_or(2.50)
            };

            let per_km_rate = if !self.per_km_rates.is_empty() {
                self.per_km_rates[rng.gen_range(0..self.per_km_rates.len())]
            } else {
                self.base.pricing_config.as_ref().map(|p| p.per_km_rate).unwrap_or(1.50)
            };

            let surge_enabled = if !self.surge_enabled.is_empty() {
                self.surge_enabled[rng.gen_range(0..self.surge_enabled.len())]
            } else {
                self.base.pricing_config.as_ref().map(|p| p.surge_enabled).unwrap_or(false)
            };

            let surge_max_multiplier = if !self.surge_max_multipliers.is_empty() {
                self.surge_max_multipliers[rng.gen_range(0..self.surge_max_multipliers.len())]
            } else {
                self.base.pricing_config.as_ref().map(|p| p.surge_max_multiplier).unwrap_or(2.0)
            };

            params.num_riders = if !self.num_riders.is_empty() {
                self.num_riders[rng.gen_range(0..self.num_riders.len())]
            } else {
                self.base.num_riders
            };

            params.num_drivers = if !self.num_drivers.is_empty() {
                self.num_drivers[rng.gen_range(0..self.num_drivers.len())]
            } else {
                self.base.num_drivers
            };

            params.match_radius = if !self.match_radii.is_empty() {
                self.match_radii[rng.gen_range(0..self.match_radii.len())]
            } else {
                self.base.match_radius
            };

            params.epoch_ms = if !self.epoch_ms.is_empty() {
                self.epoch_ms[rng.gen_range(0..self.epoch_ms.len())]
            } else {
                self.base.epoch_ms
            };

            // Sample simulation duration and set end time
            let simulation_duration_hours = if !self.simulation_duration_hours.is_empty() {
                self.simulation_duration_hours[rng.gen_range(0..self.simulation_duration_hours.len())]
            } else {
                None
            };

            if let Some(duration_hours) = simulation_duration_hours {
                let request_window_ms = params.request_window_ms;
                let end_time_ms = request_window_ms.saturating_add(duration_hours * 60 * 60 * 1000);
                params.simulation_end_time_ms = Some(end_time_ms);
            }

            let pricing_config = PricingConfig {
                base_fare,
                per_km_rate,
                commission_rate,
                surge_enabled,
                surge_radius_k: self.base.pricing_config.as_ref().map(|p| p.surge_radius_k).unwrap_or(1),
                surge_max_multiplier,
            };
            params.pricing_config = Some(pricing_config);

            // Create a hash of the parameter combination to avoid duplicates
            let param_hash = format!("{:?}", params);
            if seen.contains(&param_hash) {
                continue;
            }
            seen.insert(param_hash);

            let seed_value = seed.wrapping_add(parameter_sets.len() as u64).wrapping_mul(0x9e3779b9);

            parameter_sets.push(ParameterSet::new(
                params,
                format!("random_{}", parameter_sets.len()),
                0,
                seed_value,
            ));
        }

        parameter_sets
    }
}

impl Default for ParameterSpace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_search_single_parameter() {
        let space = ParameterSpace::grid()
            .commission_rate(vec![0.0, 0.1, 0.2]);
        let sets = space.generate();
        assert_eq!(sets.len(), 3);
    }

    #[test]
    fn test_grid_search_multiple_parameters() {
        let space = ParameterSpace::grid()
            .commission_rate(vec![0.0, 0.1])
            .num_drivers(vec![50, 100]);
        let sets = space.generate();
        assert_eq!(sets.len(), 4); // 2 * 2 = 4 combinations
    }

    #[test]
    fn test_random_sampling() {
        let space = ParameterSpace::grid()
            .commission_rate(vec![0.0, 0.1, 0.2, 0.3])
            .num_drivers(vec![50, 100, 150]);
        let sets = space.sample_random(10, 42);
        assert_eq!(sets.len(), 10);
    }

    #[test]
    fn test_epoch_ms_and_duration() {
        let space = ParameterSpace::grid()
            .epoch_ms(vec![Some(1700000000000), Some(1700086400000)])
            .simulation_duration_hours(vec![Some(4), Some(8)]);
        let sets = space.generate();
        assert_eq!(sets.len(), 4); // 2 * 2 = 4 combinations
        
        // Verify epoch_ms is set correctly
        assert_eq!(sets[0].scenario_params().epoch_ms, Some(1700000000000));
        assert_eq!(sets[1].scenario_params().epoch_ms, Some(1700000000000));
        assert_eq!(sets[2].scenario_params().epoch_ms, Some(1700086400000));
        assert_eq!(sets[3].scenario_params().epoch_ms, Some(1700086400000));
        
        // Verify simulation_end_time_ms is set based on duration
        // Each set should have end_time = request_window_ms + duration_hours * 3600000
        let request_window = sets[0].scenario_params().request_window_ms;
        assert_eq!(sets[0].scenario_params().simulation_end_time_ms, Some(request_window + 4 * 60 * 60 * 1000));
        assert_eq!(sets[1].scenario_params().simulation_end_time_ms, Some(request_window + 8 * 60 * 60 * 1000));
    }
}
