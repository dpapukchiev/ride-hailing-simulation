use sim_core::scenario::MatchingAlgorithmType;
use sim_core::traffic::TrafficProfileKind;

use crate::metrics::SimulationResult;
use crate::parameters::ParameterSet;

pub(crate) fn export_to_csv_impl(
    results: &[SimulationResult],
    parameter_sets: &[ParameterSet],
    file: std::fs::File,
) -> Result<(), Box<dyn std::error::Error>> {
    if results.len() != parameter_sets.len() {
        return Err(format!(
            "Results length ({}) doesn't match parameter_sets length ({})",
            results.len(),
            parameter_sets.len()
        )
        .into());
    }

    let mut wtr = csv::Writer::from_writer(file);

    wtr.write_record([
        "experiment_id",
        "run_id",
        "seed",
        "commission_rate",
        "base_fare",
        "per_km_rate",
        "surge_enabled",
        "surge_radius_k",
        "surge_max_multiplier",
        "num_riders",
        "num_drivers",
        "match_radius",
        "epoch_ms",
        "matching_algorithm_type",
        "batch_matching_enabled",
        "batch_interval_secs",
        "eta_weight",
        "traffic_profile",
        "dynamic_congestion_enabled",
        "base_speed_kmh",
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
            &param_set.experiment_id,
            &param_set.run_id.to_string(),
            &param_set.seed.to_string(),
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
            &match &param_set.params.traffic_profile {
                TrafficProfileKind::None => "None".to_string(),
                TrafficProfileKind::Berlin => "Berlin".to_string(),
                TrafficProfileKind::Custom(_) => "Custom".to_string(),
            },
            &param_set.params.dynamic_congestion_enabled.to_string(),
            &param_set
                .params
                .base_speed_kmh
                .map(|s| s.to_string())
                .unwrap_or_default(),
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
