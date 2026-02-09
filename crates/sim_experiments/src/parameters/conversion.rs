use super::combinations::ParameterCombination;
use super::ParameterSet;
use sim_core::pricing::PricingConfig;
use sim_core::scenario::ScenarioParams;

pub(super) fn combination_to_parameter_set(
    base: &ScenarioParams,
    combo: ParameterCombination,
    experiment_id: usize,
) -> ParameterSet {
    let mut params = base.clone();
    params.num_riders = combo.num_riders;
    params.num_drivers = combo.num_drivers;
    params.match_radius = combo.match_radius;
    params.epoch_ms = combo.epoch_ms;

    if let Some(duration_hours) = combo.simulation_duration_hours {
        let request_window_ms = params.request_window_ms;
        let end_time_ms = request_window_ms.saturating_add(duration_hours * 60 * 60 * 1000);
        params.simulation_end_time_ms = Some(end_time_ms);
    }

    params.pricing_config = Some(PricingConfig {
        base_fare: combo.base_fare,
        per_km_rate: combo.per_km_rate,
        commission_rate: combo.commission_rate,
        surge_enabled: combo.surge_enabled,
        surge_radius_k: combo.surge_radius_k,
        surge_max_multiplier: combo.surge_max_multiplier,
    });

    params.matching_algorithm_type = Some(combo.matching_algorithm_type);
    params.batch_matching_enabled = Some(combo.batch_matching_enabled);
    params.batch_interval_secs = Some(combo.batch_interval_secs);
    params.eta_weight = Some(combo.eta_weight);

    let seed = (experiment_id as u64).wrapping_mul(0x9e3779b9);

    ParameterSet::new(params, format!("exp_{experiment_id}"), 0, seed)
}
