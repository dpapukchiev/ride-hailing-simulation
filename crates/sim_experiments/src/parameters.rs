//! Parameter variation framework for exploring simulation parameter space.
//!
//! This module provides tools for defining parameter spaces and generating
//! parameter sets for parallel experimentation. Supports grid search and
//! random sampling strategies.

use sim_core::pricing::PricingConfig;
use sim_core::scenario::{MatchingAlgorithmType, ScenarioParams};
use std::collections::HashSet;

/// Represents a single parameter combination.
#[derive(Debug, Clone)]
struct ParameterCombination {
    commission_rate: f64,
    base_fare: f64,
    per_km_rate: f64,
    surge_enabled: bool,
    surge_radius_k: u32,
    surge_max_multiplier: f64,
    num_riders: usize,
    num_drivers: usize,
    match_radius: u32,
    epoch_ms: Option<i64>,
    simulation_duration_hours: Option<u64>,
    matching_algorithm_type: MatchingAlgorithmType,
    batch_matching_enabled: bool,
    batch_interval_secs: u64,
    eta_weight: f64,
}

/// Partial combination used for incremental building.
#[derive(Debug, Clone, Default)]
struct PartialCombination {
    commission_rate: Option<f64>,
    base_fare: Option<f64>,
    per_km_rate: Option<f64>,
    surge_enabled: Option<bool>,
    surge_radius_k: Option<u32>,
    surge_max_multiplier: Option<f64>,
    num_riders: Option<usize>,
    num_drivers: Option<usize>,
    match_radius: Option<u32>,
    epoch_ms: Option<Option<i64>>,
    simulation_duration_hours: Option<Option<u64>>,
    matching_algorithm_type: Option<MatchingAlgorithmType>,
    batch_matching_enabled: Option<bool>,
    batch_interval_secs: Option<u64>,
}

impl PartialCombination {
    fn with_commission_rate(mut self, v: f64) -> Self {
        self.commission_rate = Some(v);
        self
    }
    fn with_base_fare(mut self, v: f64) -> Self {
        self.base_fare = Some(v);
        self
    }
    fn with_per_km_rate(mut self, v: f64) -> Self {
        self.per_km_rate = Some(v);
        self
    }
    fn with_surge_enabled(mut self, v: bool) -> Self {
        self.surge_enabled = Some(v);
        self
    }
    fn with_surge_radius_k(mut self, v: u32) -> Self {
        self.surge_radius_k = Some(v);
        self
    }
    fn with_surge_max_multiplier(mut self, v: f64) -> Self {
        self.surge_max_multiplier = Some(v);
        self
    }
    fn with_num_riders(mut self, v: usize) -> Self {
        self.num_riders = Some(v);
        self
    }
    fn with_num_drivers(mut self, v: usize) -> Self {
        self.num_drivers = Some(v);
        self
    }
    fn with_match_radius(mut self, v: u32) -> Self {
        self.match_radius = Some(v);
        self
    }
    fn with_epoch_ms(mut self, v: Option<i64>) -> Self {
        self.epoch_ms = Some(v);
        self
    }
    fn with_simulation_duration_hours(mut self, v: Option<u64>) -> Self {
        self.simulation_duration_hours = Some(v);
        self
    }
    fn with_matching_algorithm_type(mut self, v: MatchingAlgorithmType) -> Self {
        self.matching_algorithm_type = Some(v);
        self
    }
    fn with_batch_matching_enabled(mut self, v: bool) -> Self {
        self.batch_matching_enabled = Some(v);
        self
    }
    fn with_batch_interval_secs(mut self, v: u64) -> Self {
        self.batch_interval_secs = Some(v);
        self
    }
    fn into_combination(self, eta_weight: f64) -> ParameterCombination {
        ParameterCombination {
            commission_rate: self.commission_rate.unwrap(),
            base_fare: self.base_fare.unwrap(),
            per_km_rate: self.per_km_rate.unwrap(),
            surge_enabled: self.surge_enabled.unwrap(),
            surge_radius_k: self.surge_radius_k.unwrap(),
            surge_max_multiplier: self.surge_max_multiplier.unwrap(),
            num_riders: self.num_riders.unwrap(),
            num_drivers: self.num_drivers.unwrap(),
            match_radius: self.match_radius.unwrap(),
            epoch_ms: self.epoch_ms.unwrap(),
            simulation_duration_hours: self.simulation_duration_hours.unwrap(),
            matching_algorithm_type: self.matching_algorithm_type.unwrap(),
            batch_matching_enabled: self.batch_matching_enabled.unwrap(),
            batch_interval_secs: self.batch_interval_secs.unwrap(),
            eta_weight,
        }
    }
}

/// Holds all parameter variations to explore.
struct ParameterVariations {
    commission_rates: Vec<f64>,
    base_fares: Vec<f64>,
    per_km_rates: Vec<f64>,
    surge_enabled: Vec<bool>,
    surge_radius_k: Vec<u32>,
    surge_max_multipliers: Vec<f64>,
    num_riders: Vec<usize>,
    num_drivers: Vec<usize>,
    match_radii: Vec<u32>,
    epoch_ms_values: Vec<Option<i64>>,
    simulation_duration_hours: Vec<Option<u64>>,
    matching_algorithm_types: Vec<MatchingAlgorithmType>,
    batch_matching_enabled: Vec<bool>,
    batch_interval_secs: Vec<u64>,
    eta_weights: Vec<f64>,
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
            surge_radius_k: if space.surge_radius_k.is_empty() {
                vec![default_pricing.map(|p| p.surge_radius_k).unwrap_or(1)]
            } else {
                space.surge_radius_k.clone()
            },
            surge_max_multipliers: if space.surge_max_multipliers.is_empty() {
                vec![default_pricing
                    .map(|p| p.surge_max_multiplier)
                    .unwrap_or(2.0)]
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
            matching_algorithm_types: if space.matching_algorithm_types.is_empty() {
                vec![space
                    .base
                    .matching_algorithm_type
                    .unwrap_or(MatchingAlgorithmType::Hungarian)]
            } else {
                space.matching_algorithm_types.clone()
            },
            batch_matching_enabled: if space.batch_matching_enabled.is_empty() {
                vec![space.base.batch_matching_enabled.unwrap_or(true)]
            } else {
                space.batch_matching_enabled.clone()
            },
            batch_interval_secs: if space.batch_interval_secs.is_empty() {
                vec![space.base.batch_interval_secs.unwrap_or(5)]
            } else {
                space.batch_interval_secs.clone()
            },
            eta_weights: if space.eta_weights.is_empty() {
                vec![space
                    .base
                    .eta_weight
                    .unwrap_or(sim_core::matching::DEFAULT_ETA_WEIGHT)]
            } else {
                space.eta_weights.clone()
            },
        }
    }

    /// Generate all combinations using Cartesian product.
    /// Uses a functional fold-based approach to build combinations incrementally.
    fn generate_combinations(&self) -> Vec<ParameterCombination> {
        // Start with a single empty combination
        let mut partial: Vec<PartialCombination> = vec![PartialCombination::default()];

        // Build combinations incrementally by expanding each partial combination
        partial = self
            .commission_rates
            .iter()
            .flat_map(|&rate| {
                partial
                    .iter()
                    .map(move |p| p.clone().with_commission_rate(rate))
            })
            .collect();

        partial = self
            .base_fares
            .iter()
            .flat_map(|&fare| partial.iter().map(move |p| p.clone().with_base_fare(fare)))
            .collect();

        partial = self
            .per_km_rates
            .iter()
            .flat_map(|&rate| {
                partial
                    .iter()
                    .map(move |p| p.clone().with_per_km_rate(rate))
            })
            .collect();

        partial = self
            .surge_enabled
            .iter()
            .flat_map(|&enabled| {
                partial
                    .iter()
                    .map(move |p| p.clone().with_surge_enabled(enabled))
            })
            .collect();

        partial = self
            .surge_radius_k
            .iter()
            .flat_map(|&radius_k| {
                partial
                    .iter()
                    .map(move |p| p.clone().with_surge_radius_k(radius_k))
            })
            .collect();

        partial = self
            .surge_max_multipliers
            .iter()
            .flat_map(|&mult| {
                partial
                    .iter()
                    .map(move |p| p.clone().with_surge_max_multiplier(mult))
            })
            .collect();

        partial = self
            .num_riders
            .iter()
            .flat_map(|&count| {
                partial
                    .iter()
                    .map(move |p| p.clone().with_num_riders(count))
            })
            .collect();

        partial = self
            .num_drivers
            .iter()
            .flat_map(|&count| {
                partial
                    .iter()
                    .map(move |p| p.clone().with_num_drivers(count))
            })
            .collect();

        partial = self
            .match_radii
            .iter()
            .flat_map(|&radius| {
                partial
                    .iter()
                    .map(move |p| p.clone().with_match_radius(radius))
            })
            .collect();

        partial = self
            .epoch_ms_values
            .iter()
            .flat_map(|&epoch| partial.iter().map(move |p| p.clone().with_epoch_ms(epoch)))
            .collect();

        partial = self
            .simulation_duration_hours
            .iter()
            .flat_map(|&duration| {
                partial
                    .iter()
                    .map(move |p| p.clone().with_simulation_duration_hours(duration))
            })
            .collect();

        partial = self
            .matching_algorithm_types
            .iter()
            .flat_map(|&alg| {
                partial
                    .iter()
                    .map(move |p| p.clone().with_matching_algorithm_type(alg))
            })
            .collect();

        partial = self
            .batch_matching_enabled
            .iter()
            .flat_map(|&enabled| {
                partial
                    .iter()
                    .map(move |p| p.clone().with_batch_matching_enabled(enabled))
            })
            .collect();

        partial = self
            .batch_interval_secs
            .iter()
            .flat_map(|&interval| {
                partial
                    .iter()
                    .map(move |p| p.clone().with_batch_interval_secs(interval))
            })
            .collect();

        // Final step: convert to ParameterCombination
        self.eta_weights
            .iter()
            .flat_map(|&eta_weight| {
                partial
                    .iter()
                    .map(move |p| p.clone().into_combination(eta_weight))
            })
            .collect()
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
    base: ScenarioParams,
    /// Commission rates to explore.
    commission_rates: Vec<f64>,
    /// Base fares to explore.
    base_fares: Vec<f64>,
    /// Per-km rates to explore.
    per_km_rates: Vec<f64>,
    /// Surge enabled values to explore.
    surge_enabled: Vec<bool>,
    /// Surge radius k values to explore.
    surge_radius_k: Vec<u32>,
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
    /// Matching algorithm types to explore.
    matching_algorithm_types: Vec<MatchingAlgorithmType>,
    /// Batch matching enabled values to explore.
    batch_matching_enabled: Vec<bool>,
    /// Batch interval (seconds) values to explore.
    batch_interval_secs: Vec<u64>,
    /// ETA weights to explore.
    eta_weights: Vec<f64>,
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

    /// Set base parameters (used as defaults).
    pub fn with_base(mut self, base: ScenarioParams) -> Self {
        self.base = base;
        self
    }

    /// Check if a parameter combination is valid.
    ///
    /// Returns false for invalid combinations that should be discarded.
    fn is_valid_combination(combo: &ParameterCombination) -> bool {
        // Hungarian matching algorithm requires batch matching to be enabled
        // (it's designed for batch optimization via find_batch_matches)
        if combo.matching_algorithm_type == MatchingAlgorithmType::Hungarian
            && !combo.batch_matching_enabled
        {
            return false;
        }
        true
    }

    /// Generate all parameter sets using grid search (Cartesian product).
    ///
    /// Each combination of specified parameters will be generated.
    /// Parameters not specified will use values from the base configuration.
    /// Invalid combinations (e.g., Hungarian matching without batch matching) are filtered out.
    pub fn generate(&self) -> Vec<ParameterSet> {
        // Collect all parameter variations with defaults
        let variations = ParameterVariations::from_space(self);

        // Generate Cartesian product using nested loops (avoids stack overflow)
        let combinations: Vec<_> = variations
            .generate_combinations()
            .into_iter()
            .filter(Self::is_valid_combination)
            .collect();

        combinations
            .into_iter()
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
                    let end_time_ms =
                        request_window_ms.saturating_add(duration_hours * 60 * 60 * 1000);
                    params.simulation_end_time_ms = Some(end_time_ms);
                }

                let pricing_config = PricingConfig {
                    base_fare: combo.base_fare,
                    per_km_rate: combo.per_km_rate,
                    commission_rate: combo.commission_rate,
                    surge_enabled: combo.surge_enabled,
                    surge_radius_k: combo.surge_radius_k,
                    surge_max_multiplier: combo.surge_max_multiplier,
                };
                params.pricing_config = Some(pricing_config);

                // Set matching algorithm parameters
                params.matching_algorithm_type = Some(combo.matching_algorithm_type);
                params.batch_matching_enabled = Some(combo.batch_matching_enabled);
                params.batch_interval_secs = Some(combo.batch_interval_secs);
                params.eta_weight = Some(combo.eta_weight);

                let seed = (experiment_id as u64).wrapping_mul(0x9e3779b9);

                ParameterSet::new(params, format!("exp_{}", experiment_id), 0, seed)
            })
            .collect()
    }

    /// Generate random parameter sets (Monte Carlo sampling).
    ///
    /// Samples `count` parameter sets randomly from the defined space.
    /// Requires at least one parameter to have multiple values.
    /// If duplicates are encountered, continues sampling until `count` unique sets are generated.
    pub fn sample_random(&self, count: usize, seed: u64) -> Vec<ParameterSet> {
        use rand::rngs::StdRng;
        use rand::Rng;
        use rand::SeedableRng;

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
                self.base
                    .pricing_config
                    .as_ref()
                    .map(|p| p.commission_rate)
                    .unwrap_or(0.0)
            };

            let base_fare = if !self.base_fares.is_empty() {
                self.base_fares[rng.gen_range(0..self.base_fares.len())]
            } else {
                self.base
                    .pricing_config
                    .as_ref()
                    .map(|p| p.base_fare)
                    .unwrap_or(2.50)
            };

            let per_km_rate = if !self.per_km_rates.is_empty() {
                self.per_km_rates[rng.gen_range(0..self.per_km_rates.len())]
            } else {
                self.base
                    .pricing_config
                    .as_ref()
                    .map(|p| p.per_km_rate)
                    .unwrap_or(1.50)
            };

            let surge_enabled = if !self.surge_enabled.is_empty() {
                self.surge_enabled[rng.gen_range(0..self.surge_enabled.len())]
            } else {
                self.base
                    .pricing_config
                    .as_ref()
                    .map(|p| p.surge_enabled)
                    .unwrap_or(false)
            };

            let surge_radius_k = if !self.surge_radius_k.is_empty() {
                self.surge_radius_k[rng.gen_range(0..self.surge_radius_k.len())]
            } else {
                self.base
                    .pricing_config
                    .as_ref()
                    .map(|p| p.surge_radius_k)
                    .unwrap_or(1)
            };

            let surge_max_multiplier = if !self.surge_max_multipliers.is_empty() {
                self.surge_max_multipliers[rng.gen_range(0..self.surge_max_multipliers.len())]
            } else {
                self.base
                    .pricing_config
                    .as_ref()
                    .map(|p| p.surge_max_multiplier)
                    .unwrap_or(2.0)
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
                self.simulation_duration_hours
                    [rng.gen_range(0..self.simulation_duration_hours.len())]
            } else {
                None
            };

            if let Some(duration_hours) = simulation_duration_hours {
                let request_window_ms = params.request_window_ms;
                let end_time_ms = request_window_ms.saturating_add(duration_hours * 60 * 60 * 1000);
                params.simulation_end_time_ms = Some(end_time_ms);
            }

            // Sample matching algorithm parameters
            params.matching_algorithm_type = if !self.matching_algorithm_types.is_empty() {
                Some(
                    self.matching_algorithm_types
                        [rng.gen_range(0..self.matching_algorithm_types.len())],
                )
            } else {
                self.base
                    .matching_algorithm_type
                    .or(Some(MatchingAlgorithmType::Hungarian))
            };

            params.batch_matching_enabled = if !self.batch_matching_enabled.is_empty() {
                Some(
                    self.batch_matching_enabled
                        [rng.gen_range(0..self.batch_matching_enabled.len())],
                )
            } else {
                self.base.batch_matching_enabled.or(Some(true))
            };

            params.batch_interval_secs = if !self.batch_interval_secs.is_empty() {
                Some(self.batch_interval_secs[rng.gen_range(0..self.batch_interval_secs.len())])
            } else {
                self.base.batch_interval_secs.or(Some(5))
            };

            params.eta_weight = if !self.eta_weights.is_empty() {
                Some(self.eta_weights[rng.gen_range(0..self.eta_weights.len())])
            } else {
                self.base
                    .eta_weight
                    .or(Some(sim_core::matching::DEFAULT_ETA_WEIGHT))
            };

            let pricing_config = PricingConfig {
                base_fare,
                per_km_rate,
                commission_rate,
                surge_enabled,
                surge_radius_k,
                surge_max_multiplier,
            };
            params.pricing_config = Some(pricing_config);

            // Validate combination: Hungarian matching requires batch matching to be enabled
            if params.matching_algorithm_type == Some(MatchingAlgorithmType::Hungarian)
                && params.batch_matching_enabled == Some(false)
            {
                // Skip invalid combination: Hungarian requires batch matching
                continue;
            }

            // Create a hash of the parameter combination to avoid duplicates
            let param_hash = format!("{:?}", params);
            if seen.contains(&param_hash) {
                continue;
            }
            seen.insert(param_hash);

            let seed_value = seed
                .wrapping_add(parameter_sets.len() as u64)
                .wrapping_mul(0x9e3779b9);

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
        let space = ParameterSpace::grid().commission_rate(vec![0.0, 0.1, 0.2]);
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

        // Verify all combinations exist (order-independent check)
        let epoch1 = Some(1700000000000);
        let epoch2 = Some(1700086400000);
        let mut found_epoch1_dur4 = false;
        let mut found_epoch1_dur8 = false;
        let mut found_epoch2_dur4 = false;
        let mut found_epoch2_dur8 = false;

        let request_window = sets[0].scenario_params().request_window_ms;

        for set in &sets {
            let params = set.scenario_params();
            let duration_hours = params
                .simulation_end_time_ms
                .map(|end_time| (end_time - request_window) / (60 * 60 * 1000));

            match (params.epoch_ms, duration_hours) {
                (e, Some(4)) if e == epoch1 => found_epoch1_dur4 = true,
                (e, Some(8)) if e == epoch1 => found_epoch1_dur8 = true,
                (e, Some(4)) if e == epoch2 => found_epoch2_dur4 = true,
                (e, Some(8)) if e == epoch2 => found_epoch2_dur8 = true,
                _ => {}
            }

            // Verify simulation_end_time_ms is set correctly
            if let Some(dur) = duration_hours {
                let expected_end = request_window + dur * 60 * 60 * 1000;
                assert_eq!(params.simulation_end_time_ms, Some(expected_end));
            }
        }

        assert!(
            found_epoch1_dur4,
            "Missing combination: epoch1 + duration 4"
        );
        assert!(
            found_epoch1_dur8,
            "Missing combination: epoch1 + duration 8"
        );
        assert!(
            found_epoch2_dur4,
            "Missing combination: epoch2 + duration 4"
        );
        assert!(
            found_epoch2_dur8,
            "Missing combination: epoch2 + duration 8"
        );
    }

    #[test]
    fn test_invalid_combinations_filtered() {
        // Test that Hungarian matching without batch matching is filtered out
        let space = ParameterSpace::grid()
            .matching_algorithm_type(vec![
                MatchingAlgorithmType::Simple,
                MatchingAlgorithmType::Hungarian,
            ])
            .batch_matching_enabled(vec![false, true]);

        let sets = space.generate();
        // Should have: Simple+false, Simple+true, Hungarian+true (Hungarian+false filtered out)
        assert_eq!(sets.len(), 3);

        // Verify no Hungarian+false combinations exist
        for set in &sets {
            if set.params.matching_algorithm_type == Some(MatchingAlgorithmType::Hungarian) {
                assert_eq!(
                    set.params.batch_matching_enabled,
                    Some(true),
                    "Hungarian matching must have batch matching enabled"
                );
            }
        }
    }
}
