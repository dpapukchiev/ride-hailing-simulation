//! Metrics extraction from simulation results.
//!
//! This module extracts comprehensive metrics from completed simulations,
//! including conversion rates, revenue, driver payouts, and timing statistics.

use bevy_ecs::prelude::World;
use sim_core::ecs::DriverEarnings;
use sim_core::telemetry::SimTelemetry;

/// Aggregated metrics from a single simulation run.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SimulationResult {
    /// Total number of riders spawned.
    pub total_riders: usize,
    /// Total number of drivers spawned.
    pub total_drivers: usize,
    /// Number of riders who completed trips.
    pub completed_riders: usize,
    /// Number of riders who abandoned after quote rejections.
    pub abandoned_quote_riders: usize,
    /// Number of riders who cancelled during pickup wait.
    pub cancelled_riders: usize,
    /// Conversion rate (completed / total resolved).
    pub conversion_rate: f64,
    /// Total platform revenue from commissions.
    pub platform_revenue: f64,
    /// Total driver payouts (sum of all driver earnings).
    pub driver_payouts: f64,
    /// Total fares collected from riders.
    pub total_fares_collected: f64,
    /// Average time to match in milliseconds.
    pub avg_time_to_match_ms: f64,
    /// Median time to match in milliseconds.
    pub median_time_to_match_ms: f64,
    /// P90 time to match in milliseconds.
    pub p90_time_to_match_ms: f64,
    /// Average time to pickup in milliseconds.
    pub avg_time_to_pickup_ms: f64,
    /// Median time to pickup in milliseconds.
    pub median_time_to_pickup_ms: f64,
    /// P90 time to pickup in milliseconds.
    pub p90_time_to_pickup_ms: f64,
    /// Total number of completed trips.
    pub completed_trips: usize,
    /// Breakdown of abandonment reasons.
    pub riders_abandoned_price: usize,
    pub riders_abandoned_eta: usize,
    pub riders_abandoned_stochastic: usize,
}

impl SimulationResult {
    /// Calculate statistics from a vector of values.
    fn calculate_stats(values: &[u64]) -> (f64, f64, f64) {
        if values.is_empty() {
            return (0.0, 0.0, 0.0);
        }

        let mut sorted = values.to_vec();
        sorted.sort();

        let avg = sorted.iter().sum::<u64>() as f64 / sorted.len() as f64;
        let median = if sorted.len() % 2 == 0 {
            (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) as f64 / 2.0
        } else {
            sorted[sorted.len() / 2] as f64
        };
        // P90: 90th percentile - use floor(0.9 * (n-1)) for more standard percentile calculation
        let p90_idx = ((sorted.len() - 1) as f64 * 0.9) as usize;
        let p90 = sorted[p90_idx.min(sorted.len() - 1)] as f64;

        (avg, median, p90)
    }
}

/// Extract metrics from a completed simulation world.
///
/// Queries the world for telemetry data and driver earnings to compute
/// comprehensive metrics including conversion rates, revenue, payouts,
/// and timing statistics.
pub fn extract_metrics(world: &mut World) -> SimulationResult {
    // Extract telemetry data first (immutable borrow)
    let (riders_completed_total, riders_cancelled_total, riders_abandoned_quote_total,
         platform_revenue_total, total_fares_collected,
         riders_abandoned_price, riders_abandoned_eta, riders_abandoned_stochastic,
         completed_trips_data) = {
        let telemetry = world
            .get_resource::<SimTelemetry>()
            .expect("SimTelemetry resource not found");
        
        // Clone the completed trips data we need
        let trips_data: Vec<(u64, u64, u64)> = telemetry.completed_trips.iter()
            .map(|trip| (trip.requested_at, trip.matched_at, trip.pickup_at))
            .collect();
        
        (
            telemetry.riders_completed_total,
            telemetry.riders_cancelled_total,
            telemetry.riders_abandoned_quote_total,
            telemetry.platform_revenue_total,
            telemetry.total_fares_collected,
            telemetry.riders_abandoned_price,
            telemetry.riders_abandoned_eta,
            telemetry.riders_abandoned_stochastic,
            trips_data,
        )
    };

    // Now we can do mutable queries (telemetry borrow is dropped)
    let (driver_payouts, total_drivers) = {
        let drivers: Vec<_> = world.query::<&DriverEarnings>().iter(world).collect();
        let payouts: f64 = drivers.iter().map(|earnings| earnings.daily_earnings).sum();
        (payouts, drivers.len())
    };

    // Calculate total riders (completed + cancelled + abandoned)
    let total_resolved = riders_completed_total
        + riders_cancelled_total
        + riders_abandoned_quote_total;
    
    // Calculate conversion rate
    let conversion_rate = if total_resolved > 0 {
        riders_completed_total as f64 / total_resolved as f64
    } else {
        0.0
    };

    // Calculate timing statistics from completed trips
    let mut time_to_match_values: Vec<u64> = Vec::new();
    let mut time_to_pickup_values: Vec<u64> = Vec::new();

    for (requested_at, matched_at, pickup_at) in &completed_trips_data {
        time_to_match_values.push(matched_at.saturating_sub(*requested_at));
        time_to_pickup_values.push(pickup_at.saturating_sub(*matched_at));
    }

    let (avg_time_to_match, median_time_to_match, p90_time_to_match) =
        SimulationResult::calculate_stats(&time_to_match_values);
    let (avg_time_to_pickup, median_time_to_pickup, p90_time_to_pickup) =
        SimulationResult::calculate_stats(&time_to_pickup_values);

    // Estimate total riders (use resolved count as proxy if we don't have exact spawn count)
    // In a real scenario, we'd track this, but for now we use resolved count
    let total_riders = total_resolved as usize;

    SimulationResult {
        total_riders,
        total_drivers,
        completed_riders: riders_completed_total as usize,
        abandoned_quote_riders: riders_abandoned_quote_total as usize,
        cancelled_riders: riders_cancelled_total as usize,
        conversion_rate,
        platform_revenue: platform_revenue_total,
        driver_payouts,
        total_fares_collected,
        avg_time_to_match_ms: avg_time_to_match,
        median_time_to_match_ms: median_time_to_match,
        p90_time_to_match_ms: p90_time_to_match,
        avg_time_to_pickup_ms: avg_time_to_pickup,
        median_time_to_pickup_ms: median_time_to_pickup,
        p90_time_to_pickup_ms: p90_time_to_pickup,
        completed_trips: completed_trips_data.len(),
        riders_abandoned_price: riders_abandoned_price as usize,
        riders_abandoned_eta: riders_abandoned_eta as usize,
        riders_abandoned_stochastic: riders_abandoned_stochastic as usize,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_stats() {
        let values = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        let (avg, median, p90) = SimulationResult::calculate_stats(&values);
        assert_eq!(avg, 55.0);
        // Median of 10 values: average of 5th (50) and 6th (60) = 55.0
        assert_eq!(median, 55.0);
        // P90: 90% of 10 = index 9, which is 90
        assert_eq!(p90, 90.0);
    }

    #[test]
    fn test_calculate_stats_empty() {
        let (avg, median, p90) = SimulationResult::calculate_stats(&[]);
        assert_eq!(avg, 0.0);
        assert_eq!(median, 0.0);
        assert_eq!(p90, 0.0);
    }
}
