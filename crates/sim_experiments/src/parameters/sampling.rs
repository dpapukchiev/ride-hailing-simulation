use super::constraints::is_valid_matching_config;
use super::{ParameterSet, ParameterSpace};
use rand::rngs::StdRng;
use rand::Rng;
use rand::SeedableRng;
use sim_core::pricing::PricingConfig;
use sim_core::scenario::MatchingAlgorithmType;
use std::collections::HashSet;

impl ParameterSpace {
    /// Generate random parameter sets (Monte Carlo sampling).
    ///
    /// Samples `count` parameter sets randomly from the defined space.
    /// Requires at least one parameter to have multiple values.
    /// If duplicates are encountered, continues sampling until `count` unique sets are generated.
    pub fn sample_random(&self, count: usize, seed: u64) -> Vec<ParameterSet> {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut parameter_sets = Vec::new();
        let mut seen = HashSet::new();
        let mut attempts = 0;
        const MAX_ATTEMPTS: usize = 10000;

        while parameter_sets.len() < count && attempts < MAX_ATTEMPTS {
            attempts += 1;
            let mut params = self.base.clone();

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

            params.pricing_config = Some(PricingConfig {
                base_fare,
                per_km_rate,
                commission_rate,
                surge_enabled,
                surge_radius_k,
                surge_max_multiplier,
            });

            if !is_valid_matching_config(
                params
                    .matching_algorithm_type
                    .unwrap_or(MatchingAlgorithmType::Hungarian),
                params.batch_matching_enabled.unwrap_or(true),
            ) {
                continue;
            }

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
