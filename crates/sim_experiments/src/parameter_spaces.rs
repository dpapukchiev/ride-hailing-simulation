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
    let era = (if adjusted_y >= 0 { adjusted_y } else { adjusted_y - 399 }) / 400;
    let yoe = adjusted_y - era * 400;
    let doy = (153 * (adjusted_m - 3) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    
    // Add time components
    let total_secs = days * 86400 + hour as i64 * 3600 + minute as i64 * 60;
    total_secs * 1000 // Convert to milliseconds
}

/// Comprehensive parameter space exploring all major dimensions.
/// 
/// Explores pricing, supply/demand, matching algorithms, and timing.
pub fn comprehensive_space() -> ParameterSpace {
    ParameterSpace::grid()
        .commission_rate(vec![0.0, 0.1, 0.2, 0.3])           // 0%, 10%, 20%, 30% commission
        .base_fare(vec![2.0, 2.5, 3.0])                      // Base fare variations
        .per_km_rate(vec![1.0, 1.5, 2.0])                    // Per-km rate variations
        .surge_enabled(vec![false, true])                     // Surge pricing on/off
        .surge_max_multiplier(vec![1.5, 2.0, 2.5])           // Maximum surge multiplier
        .num_drivers(vec![50, 100, 150])                      // Low, medium, high supply
        .num_riders(vec![300, 500, 700])                      // Low, medium, high demand
        .match_radius(vec![5, 10, 15])                        // Match radius in km
        .epoch_ms(vec![
            Some(datetime_to_unix_ms(2026, 2, 7, 8, 0)),   // 2026-02-07 08:00 UTC (morning rush)
            Some(datetime_to_unix_ms(2026, 2, 7, 17, 0)),  // 2026-02-07 17:00 UTC (evening rush)
        ])
        .simulation_duration_hours(vec![Some(4), Some(8)])    // Different durations
        .matching_algorithm_type(vec![
            MatchingAlgorithmType::Simple, 
            MatchingAlgorithmType::CostBased, 
            MatchingAlgorithmType::Hungarian
        ])                                                       // Matching algorithms
        .batch_matching_enabled(vec![false, true])              // Batch matching on/off
        .batch_interval_secs(vec![5, 10, 20])                   // Batch interval variations
        .eta_weight(vec![0.0, 0.1, 0.5, 1.0])                   // ETA weight variations
}

/// Focused parameter space for pricing analysis.
/// 
/// Explores commission rates and pricing parameters with fixed supply/demand.
pub fn pricing_focused_space() -> ParameterSpace {
    ParameterSpace::grid()
        .commission_rate(vec![0.0, 0.1, 0.2, 0.3])           // 0%, 10%, 20%, 30% commission
        .base_fare(vec![2.0, 2.5, 3.0])                       // Base fare variations
        .per_km_rate(vec![1.0, 1.5, 2.0])                     // Per-km rate variations
        .surge_enabled(vec![false, true])                     // Surge pricing on/off
        .surge_max_multiplier(vec![1.5, 2.0, 2.5])            // Maximum surge multiplier
        .num_drivers(vec![100])                                // Fixed supply
        .num_riders(vec![500])                                 // Fixed demand
        .match_radius(vec![10])                                // Fixed match radius
        .matching_algorithm_type(vec![MatchingAlgorithmType::Hungarian]) // Fixed algorithm
        .batch_matching_enabled(vec![true])                    // Batch matching enabled
        .batch_interval_secs(vec![5])                          // Fixed batch interval
        .eta_weight(vec![0.1])                                 // Fixed ETA weight
}

/// Focused parameter space for matching algorithm comparison.
/// 
/// Explores different matching algorithms and batch configurations
/// with fixed pricing.
pub fn matching_focused_space() -> ParameterSpace {
    ParameterSpace::grid()
        .commission_rate(vec![0.2])                           // Fixed commission
        .base_fare(vec![2.5])                                  // Fixed base fare
        .per_km_rate(vec![1.5])                                // Fixed per-km rate
        .surge_enabled(vec![false])                            // No surge
        .surge_max_multiplier(vec![2.0])                       // Fixed multiplier
        .num_drivers(vec![100])                                // Fixed supply
        .num_riders(vec![500])                                 // Fixed demand
        .match_radius(vec![10])                                // Fixed match radius
        .matching_algorithm_type(vec![
            MatchingAlgorithmType::Simple, 
            MatchingAlgorithmType::CostBased, 
            MatchingAlgorithmType::Hungarian
        ])                                                       // Matching algorithms
        .batch_matching_enabled(vec![false, true])              // Batch matching on/off
        .batch_interval_secs(vec![5, 10, 20])                   // Batch interval variations
        .eta_weight(vec![0.0, 0.1, 0.5, 1.0])                   // ETA weight variations
}

/// Focused parameter space for supply/demand analysis.
/// 
/// Explores different supply and demand levels with fixed pricing and matching.
pub fn supply_demand_space() -> ParameterSpace {
    ParameterSpace::grid()
        .commission_rate(vec![0.2])                            // Fixed commission
        .base_fare(vec![2.5])                                  // Fixed base fare
        .per_km_rate(vec![1.5])                                // Fixed per-km rate
        .surge_enabled(vec![false])                            // No surge
        .surge_max_multiplier(vec![2.0])                       // Fixed multiplier
        .num_drivers(vec![50, 100, 150])                       // Low, medium, high supply
        .num_riders(vec![300, 500, 700])                       // Low, medium, high demand
        .match_radius(vec![10])                                // Fixed match radius
        .matching_algorithm_type(vec![MatchingAlgorithmType::Hungarian]) // Fixed algorithm
        .batch_matching_enabled(vec![true])                    // Batch matching enabled
        .batch_interval_secs(vec![5])                          // Fixed batch interval
        .eta_weight(vec![0.1])                                 // Fixed ETA weight
}

/// Minimal parameter space for quick testing.
/// 
/// Small space for quick validation runs.
pub fn minimal_space() -> ParameterSpace {
    ParameterSpace::grid()
        .commission_rate(vec![0.0, 0.2])                       // Two commission rates
        .num_drivers(vec![100])                                // Fixed supply
        .num_riders(vec![500])                                 // Fixed demand
        .match_radius(vec![10])                                // Fixed match radius
        .matching_algorithm_type(vec![MatchingAlgorithmType::Hungarian]) // Fixed algorithm
        .batch_matching_enabled(vec![true])                    // Batch matching enabled
        .batch_interval_secs(vec![5])                          // Fixed batch interval
        .eta_weight(vec![0.1])                                 // Fixed ETA weight
}
