//! Marketplace health score calculation.
//!
//! This module provides functions to calculate weighted health scores
//! from simulation results, combining multiple metrics into a single
//! score that represents overall marketplace health.

use crate::metrics::SimulationResult;

/// Configurable weights for marketplace health score calculation.
///
/// Each weight determines the contribution of a metric to the overall
///// health score. Higher weights mean that metric has more influence.
///
/// # Default Weights
///
/// - Conversion: 0.3 (30%)
/// - Revenue: 0.25 (25%)
/// - Driver payouts: 0.15 (15%)
/// - Time to match: 0.15 (15%, inverted - lower is better)
/// - Time to pickup: 0.15 (15%, inverted - lower is better)
/// - Abandoned rides: -0.2 (20% penalty - lower is better)
#[derive(Debug, Clone, Copy)]
pub struct HealthWeights {
    /// Weight for conversion rate (higher is better).
    pub conversion_weight: f64,
    /// Weight for platform revenue (higher is better).
    pub revenue_weight: f64,
    /// Weight for driver payouts (higher is better).
    pub driver_payouts_weight: f64,
    /// Weight for time to match (inverted - lower is better).
    pub time_to_match_weight: f64,
    /// Weight for time to pickup (inverted - lower is better).
    pub time_to_pickup_weight: f64,
    /// Penalty weight for abandoned rides (negative - lower is better).
    pub abandoned_penalty: f64,
}

impl Default for HealthWeights {
    fn default() -> Self {
        Self {
            conversion_weight: 0.3,
            revenue_weight: 0.25,
            driver_payouts_weight: 0.15,
            time_to_match_weight: 0.15,
            time_to_pickup_weight: 0.15,
            abandoned_penalty: -0.2,
        }
    }
}

impl HealthWeights {
    /// Create custom health weights.
    pub fn new(
        conversion_weight: f64,
        revenue_weight: f64,
        driver_payouts_weight: f64,
        time_to_match_weight: f64,
        time_to_pickup_weight: f64,
        abandoned_penalty: f64,
    ) -> Self {
        Self {
            conversion_weight,
            revenue_weight,
            driver_payouts_weight,
            time_to_match_weight,
            time_to_pickup_weight,
            abandoned_penalty,
        }
    }
}

/// Normalize a metric value to [0, 1] range.
///
/// Uses min-max normalization: `(value - min) / (max - min)`.
/// If min == max, returns 0.5.
fn normalize_metric(value: f64, min: f64, max: f64) -> f64 {
    if max == min {
        0.5
    } else {
        ((value - min) / (max - min)).max(0.0).min(1.0)
    }
}

/// Calculate health scores for all simulation results.
///
/// Normalizes metrics across all results and calculates weighted health scores.
/// Higher scores indicate healthier marketplace outcomes.
///
/// # Arguments
///
/// * `results` - Vector of simulation results to score
/// * `weights` - Weights for each metric component
///
/// # Returns
///
/// Vector of health scores in the same order as input results.
pub fn calculate_health_scores(
    results: &[SimulationResult],
    weights: &HealthWeights,
) -> Vec<f64> {
    if results.is_empty() {
        return vec![];
    }

    // Find min/max for each metric across all results
    let (conversion_min, conversion_max) = results
        .iter()
        .map(|r| r.conversion_rate)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), v| {
            (min.min(v), max.max(v))
        });

    let (revenue_min, revenue_max) = results
        .iter()
        .map(|r| r.platform_revenue)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), v| {
            (min.min(v), max.max(v))
        });

    let (payouts_min, payouts_max) = results
        .iter()
        .map(|r| r.driver_payouts)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), v| {
            (min.min(v), max.max(v))
        });

    let (match_time_min, match_time_max) = results
        .iter()
        .map(|r| r.avg_time_to_match_ms)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), v| {
            (min.min(v), max.max(v))
        });

    let (pickup_time_min, pickup_time_max) = results
        .iter()
        .map(|r| r.avg_time_to_pickup_ms)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), v| {
            (min.min(v), max.max(v))
        });

    let (abandoned_min, abandoned_max) = results
        .iter()
        .map(|r| r.abandoned_quote_riders as f64)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), v| {
            (min.min(v), max.max(v))
        });

    // Calculate health score for each result
    results
        .iter()
        .map(|result| {
            // Normalize metrics (higher is better for conversion, revenue, payouts)
            let conversion_norm = normalize_metric(
                result.conversion_rate,
                conversion_min,
                conversion_max,
            );
            let revenue_norm = normalize_metric(
                result.platform_revenue,
                revenue_min,
                revenue_max,
            );
            let payouts_norm = normalize_metric(
                result.driver_payouts,
                payouts_min,
                payouts_max,
            );

            // Normalize timing metrics (lower is better, so invert)
            let match_time_norm = 1.0 - normalize_metric(
                result.avg_time_to_match_ms,
                match_time_min,
                match_time_max,
            );
            let pickup_time_norm = 1.0 - normalize_metric(
                result.avg_time_to_pickup_ms,
                pickup_time_min,
                pickup_time_max,
            );

            // Normalize abandoned rides (lower is better, so invert)
            let abandoned_norm = 1.0 - normalize_metric(
                result.abandoned_quote_riders as f64,
                abandoned_min,
                abandoned_max,
            );

            // Calculate weighted sum
            conversion_norm * weights.conversion_weight
                + revenue_norm * weights.revenue_weight
                + payouts_norm * weights.driver_payouts_weight
                + match_time_norm * weights.time_to_match_weight
                + pickup_time_norm * weights.time_to_pickup_weight
                + abandoned_norm * weights.abandoned_penalty
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::SimulationResult;

    #[test]
    fn test_normalize_metric() {
        assert_eq!(normalize_metric(50.0, 0.0, 100.0), 0.5);
        assert_eq!(normalize_metric(0.0, 0.0, 100.0), 0.0);
        assert_eq!(normalize_metric(100.0, 0.0, 100.0), 1.0);
        assert_eq!(normalize_metric(50.0, 50.0, 50.0), 0.5); // min == max case
    }

    #[test]
    fn test_calculate_health_scores() {
        let results = vec![
            SimulationResult {
                total_riders: 100,
                total_drivers: 20,
                completed_riders: 80,
                abandoned_quote_riders: 10,
                cancelled_riders: 10,
                conversion_rate: 0.8,
                platform_revenue: 1000.0,
                driver_payouts: 5000.0,
                total_fares_collected: 6000.0,
                avg_time_to_match_ms: 1000.0,
                median_time_to_match_ms: 1000.0,
                p90_time_to_match_ms: 2000.0,
                avg_time_to_pickup_ms: 5000.0,
                median_time_to_pickup_ms: 5000.0,
                p90_time_to_pickup_ms: 10000.0,
                completed_trips: 80,
                riders_abandoned_price: 5,
                riders_abandoned_eta: 3,
                riders_abandoned_stochastic: 2,
            },
            SimulationResult {
                total_riders: 100,
                total_drivers: 15,
                completed_riders: 60,
                abandoned_quote_riders: 30,
                cancelled_riders: 10,
                conversion_rate: 0.6,
                platform_revenue: 500.0,
                driver_payouts: 2500.0,
                total_fares_collected: 3000.0,
                avg_time_to_match_ms: 2000.0,
                median_time_to_match_ms: 2000.0,
                p90_time_to_match_ms: 4000.0,
                avg_time_to_pickup_ms: 10000.0,
                median_time_to_pickup_ms: 10000.0,
                p90_time_to_pickup_ms: 20000.0,
                completed_trips: 60,
                riders_abandoned_price: 15,
                riders_abandoned_eta: 10,
                riders_abandoned_stochastic: 5,
            },
        ];

        let weights = HealthWeights::default();
        let scores = calculate_health_scores(&results, &weights);

        assert_eq!(scores.len(), 2);
        // First result should have higher score (better metrics)
        assert!(scores[0] > scores[1]);
    }

    #[test]
    fn test_calculate_health_scores_empty() {
        let scores = calculate_health_scores(&[], &HealthWeights::default());
        assert_eq!(scores.len(), 0);
    }
}
