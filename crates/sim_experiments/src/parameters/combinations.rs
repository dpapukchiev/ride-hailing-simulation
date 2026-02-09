use super::ParameterSpace;
use sim_core::scenario::MatchingAlgorithmType;

/// Represents a single parameter combination.
#[derive(Debug, Clone)]
pub(super) struct ParameterCombination {
    pub(super) commission_rate: f64,
    pub(super) base_fare: f64,
    pub(super) per_km_rate: f64,
    pub(super) surge_enabled: bool,
    pub(super) surge_radius_k: u32,
    pub(super) surge_max_multiplier: f64,
    pub(super) num_riders: usize,
    pub(super) num_drivers: usize,
    pub(super) match_radius: u32,
    pub(super) epoch_ms: Option<i64>,
    pub(super) simulation_duration_hours: Option<u64>,
    pub(super) matching_algorithm_type: MatchingAlgorithmType,
    pub(super) batch_matching_enabled: bool,
    pub(super) batch_interval_secs: u64,
    pub(super) eta_weight: f64,
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
    fn with_commission_rate(mut self, value: f64) -> Self {
        self.commission_rate = Some(value);
        self
    }

    fn with_base_fare(mut self, value: f64) -> Self {
        self.base_fare = Some(value);
        self
    }

    fn with_per_km_rate(mut self, value: f64) -> Self {
        self.per_km_rate = Some(value);
        self
    }

    fn with_surge_enabled(mut self, value: bool) -> Self {
        self.surge_enabled = Some(value);
        self
    }

    fn with_surge_radius_k(mut self, value: u32) -> Self {
        self.surge_radius_k = Some(value);
        self
    }

    fn with_surge_max_multiplier(mut self, value: f64) -> Self {
        self.surge_max_multiplier = Some(value);
        self
    }

    fn with_num_riders(mut self, value: usize) -> Self {
        self.num_riders = Some(value);
        self
    }

    fn with_num_drivers(mut self, value: usize) -> Self {
        self.num_drivers = Some(value);
        self
    }

    fn with_match_radius(mut self, value: u32) -> Self {
        self.match_radius = Some(value);
        self
    }

    fn with_epoch_ms(mut self, value: Option<i64>) -> Self {
        self.epoch_ms = Some(value);
        self
    }

    fn with_simulation_duration_hours(mut self, value: Option<u64>) -> Self {
        self.simulation_duration_hours = Some(value);
        self
    }

    fn with_matching_algorithm_type(mut self, value: MatchingAlgorithmType) -> Self {
        self.matching_algorithm_type = Some(value);
        self
    }

    fn with_batch_matching_enabled(mut self, value: bool) -> Self {
        self.batch_matching_enabled = Some(value);
        self
    }

    fn with_batch_interval_secs(mut self, value: u64) -> Self {
        self.batch_interval_secs = Some(value);
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
pub(super) struct ParameterVariations {
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
    pub(super) fn from_space(space: &ParameterSpace) -> Self {
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
                vec![None]
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

    pub(super) fn generate_combinations(&self) -> Vec<ParameterCombination> {
        let mut partial: Vec<PartialCombination> = vec![PartialCombination::default()];

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
            .flat_map(|&algorithm| {
                partial
                    .iter()
                    .map(move |p| p.clone().with_matching_algorithm_type(algorithm))
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
