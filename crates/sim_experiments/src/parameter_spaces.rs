//! Pre-defined parameter space configurations for experimentation.
//!
//! This module provides ready-to-use parameter space definitions that can be
//! easily selected for different types of experiments.

use crate::ParameterSpace;
use sim_core::scenario::MatchingAlgorithmType;

/// Convert a human-readable date/time to Unix epoch milliseconds (UTC).
///
/// # Arguments
/// * `year` - Year (e.g., 2026)
/// * `month` - Month (1-12)
/// * `day` - Day of month (1-31)
/// * `hour` - Hour (0-23)
/// * `minute` - Minute (0-59)
fn datetime_to_unix_ms(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> i64 {
    // Convert date to days since Unix epoch
    // Algorithm from https://howardhinnant.github.io/date_algorithms.html
    let y = year as i64;
    let m = month as i64;
    let d = day as i64;

    // Adjust for month
    let adjusted_m = if m <= 2 { m + 12 } else { m };
    let adjusted_y = if m <= 2 { y - 1 } else { y };

    // Calculate days since epoch (1970-01-01)
    let era = (if adjusted_y >= 0 {
        adjusted_y
    } else {
        adjusted_y - 399
    }) / 400;
    let yoe = adjusted_y - era * 400;
    let doy = (153 * (adjusted_m - 3) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;

    // Add time components
    let total_secs = days * 86400 + hour as i64 * 3600 + minute as i64 * 60;
    total_secs * 1000 // Convert to milliseconds
}

pub fn comprehensive_space() -> ParameterSpace {
    ParameterSpace::grid()
        .commission_rate(vec![0.0, 0.1, 0.2, 0.3])
        .base_fare(vec![2.0, 2.5, 3.0])
        .per_km_rate(vec![1.0, 1.5, 2.0])
        .surge_enabled(vec![false, true])
        .surge_max_multiplier(vec![1.5, 2.0, 2.5])
        .num_drivers(vec![100, 150, 250, 300, 500])
        .num_riders(vec![300, 500, 700, 1000])
        .match_radius(vec![5, 10, 15])
        .epoch_ms(vec![
            Some(datetime_to_unix_ms(2026, 2, 7, 8, 0)),
            Some(datetime_to_unix_ms(2026, 2, 7, 17, 0)),
        ])
        .simulation_duration_hours(vec![Some(8), Some(18)])
        .matching_algorithm_type(vec![
            MatchingAlgorithmType::Simple,
            MatchingAlgorithmType::CostBased,
            MatchingAlgorithmType::Hungarian,
        ])
        .batch_matching_enabled(vec![false, true])
        .batch_interval_secs(vec![5, 10, 20])
        .eta_weight(vec![0.0, 0.1, 0.5, 1.0])
}

pub fn pricing_focused_space() -> ParameterSpace {
    ParameterSpace::grid()
        .commission_rate(vec![0.1, 0.2, 0.3])
        .base_fare(vec![2.0, 2.5, 3.0])
        .per_km_rate(vec![1.0, 1.5, 2.0])
        .surge_enabled(vec![false, true])
        .surge_max_multiplier(vec![1.5, 2.0, 2.5])
        .surge_radius_k(vec![1, 2, 4])
        .num_drivers(vec![100, 200, 300])
        .num_riders(vec![500, 1000, 1500])
        .match_radius(vec![10])
        .matching_algorithm_type(vec![MatchingAlgorithmType::Hungarian])
        .batch_matching_enabled(vec![true])
        .batch_interval_secs(vec![10, 20, 30])
        .eta_weight(vec![0.1])
}

pub fn surge_pricing_space() -> ParameterSpace {
    ParameterSpace::grid()
        .commission_rate(vec![0.15, 0.2])
        .base_fare(vec![2.5])
        .per_km_rate(vec![1.2, 1.5])
        .surge_enabled(vec![true])
        .surge_max_multiplier(vec![1.5, 2.0, 2.5, 3.0])
        .surge_radius_k(vec![1, 2, 3, 4, 5])
        .num_drivers(vec![100, 125, 150, 250, 300])
        .num_riders(vec![500, 600, 700])
        .match_radius(vec![10])
        .matching_algorithm_type(vec![MatchingAlgorithmType::Hungarian])
        .batch_matching_enabled(vec![true])
        .batch_interval_secs(vec![10, 20, 30])
        .eta_weight(vec![0.1])
}

pub fn matching_focused_space() -> ParameterSpace {
    ParameterSpace::grid()
        .commission_rate(vec![0.2])
        .base_fare(vec![2.5])
        .per_km_rate(vec![1.5])
        .surge_enabled(vec![false])
        .surge_max_multiplier(vec![2.0])
        .num_drivers(vec![100])
        .num_riders(vec![500])
        .match_radius(vec![10])
        .matching_algorithm_type(vec![
            MatchingAlgorithmType::Simple,
            MatchingAlgorithmType::CostBased,
            MatchingAlgorithmType::Hungarian,
        ])
        .batch_matching_enabled(vec![false, true])
        .batch_interval_secs(vec![5, 10, 20])
        .eta_weight(vec![0.0, 0.1, 0.5, 1.0])
}

pub fn supply_demand_space() -> ParameterSpace {
    ParameterSpace::grid()
        .commission_rate(vec![0.2])
        .base_fare(vec![2.5])
        .per_km_rate(vec![1.5])
        .surge_enabled(vec![false])
        .surge_max_multiplier(vec![2.0])
        .num_drivers(vec![50, 100, 150])
        .num_riders(vec![300, 500, 700])
        .match_radius(vec![10])
        .matching_algorithm_type(vec![MatchingAlgorithmType::Hungarian])
        .batch_matching_enabled(vec![true])
        .batch_interval_secs(vec![5])
        .eta_weight(vec![0.1])
}

pub fn minimal_space() -> ParameterSpace {
    ParameterSpace::grid()
        .commission_rate(vec![0.0, 0.2])
        .num_drivers(vec![100])
        .num_riders(vec![500])
        .match_radius(vec![10])
        .matching_algorithm_type(vec![MatchingAlgorithmType::Hungarian])
        .batch_matching_enabled(vec![true])
        .batch_interval_secs(vec![5])
        .eta_weight(vec![0.1])
}

/// Fine-grained parameter space focused on top-performing configurations
/// and testing surge/commission interactions.
///
/// This space targets:
/// - Commission rates around 15-20%
/// - Driver-to-rider ratios 0.9-1.3
/// - Surge enabled only under constrained supply
/// - Hungarian and CostBased matching with batch intervals ≤5s
/// - Surge radius vs multiplier trade-offs
pub fn refined_surge_commission_space() -> ParameterSpace {
    ParameterSpace::grid()
        .commission_rate(vec![0.15, 0.17, 0.19, 0.20])
        .base_fare(vec![2.0, 2.5, 3.0])
        .per_km_rate(vec![1.0, 1.2, 1.5])
        .surge_enabled(vec![true, false])
        .surge_radius_k(vec![1, 2, 3])
        .surge_max_multiplier(vec![1.2, 1.5, 1.8])
        .num_drivers(vec![400, 500, 750])
        .num_riders(vec![600, 900, 1300])
        .match_radius(vec![5, 10])
        .matching_algorithm_type(vec![
            MatchingAlgorithmType::CostBased,
            MatchingAlgorithmType::Hungarian,
        ])
        .batch_matching_enabled(vec![true])
        .batch_interval_secs(vec![3, 5])
        .eta_weight(vec![0.0, 0.1])
        .simulation_duration_hours(vec![Some(12)])
}
