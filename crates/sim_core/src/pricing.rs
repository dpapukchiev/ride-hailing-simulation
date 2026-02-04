//! Pricing system for calculating trip fares with optional surge pricing.

use bevy_ecs::prelude::Resource;
use h3o::CellIndex;

use crate::spatial::distance_km_between_cells;

/// Base fare in currency units (e.g., dollars).
pub const BASE_FARE: f64 = 2.50;

/// Per-kilometer rate in currency units.
pub const PER_KM_RATE: f64 = 1.50;

/// Pricing configuration for the marketplace.
#[derive(Debug, Clone, Copy, Resource)]
pub struct PricingConfig {
    /// Base fare in currency units (e.g., dollars).
    pub base_fare: f64,
    /// Per-kilometer rate in currency units.
    pub per_km_rate: f64,
    /// Commission rate as a fraction (0.0-1.0). 0.15 means 15% commission.
    pub commission_rate: f64,
    /// When true, apply surge multiplier when demand exceeds supply in pickup H3 cluster.
    pub surge_enabled: bool,
    /// H3 grid disk radius (k) for surge cluster around pickup. 1 = immediate neighbors.
    pub surge_radius_k: u32,
    /// Maximum surge multiplier cap (e.g. 2.0 = 2x base fare).
    pub surge_max_multiplier: f64,
}

impl Default for PricingConfig {
    fn default() -> Self {
        Self {
            base_fare: BASE_FARE,
            per_km_rate: PER_KM_RATE,
            commission_rate: 0.0,
            surge_enabled: false,
            surge_radius_k: 1,
            surge_max_multiplier: 2.0,
        }
    }
}

/// Calculate commission amount from fare and commission rate.
pub fn calculate_commission(fare: f64, commission_rate: f64) -> f64 {
    fare * commission_rate
}

/// Calculate driver earnings (fare minus commission).
pub fn calculate_driver_earnings(fare: f64, commission_rate: f64) -> f64 {
    fare * (1.0 - commission_rate)
}

/// Calculate platform revenue (commission amount).
pub fn calculate_platform_revenue(fare: f64, commission_rate: f64) -> f64 {
    calculate_commission(fare, commission_rate)
}

/// Calculate fare for a trip based on distance.
/// 
/// Formula: `fare = base_fare + (distance_km * per_km_rate)`
/// 
/// Returns the total fare amount. Commission should be deducted separately.
pub fn calculate_trip_fare(pickup: CellIndex, dropoff: CellIndex) -> f64 {
    let distance_km = distance_km_between_cells(pickup, dropoff);
    BASE_FARE + (distance_km * PER_KM_RATE)
}

/// Calculate fare for a trip using the provided pricing config.
pub fn calculate_trip_fare_with_config(
    pickup: CellIndex,
    dropoff: CellIndex,
    config: PricingConfig,
) -> f64 {
    let distance_km = distance_km_between_cells(pickup, dropoff);
    config.base_fare + (distance_km * config.per_km_rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fare_includes_base_and_distance() {
        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
        let nearby = cell
            .grid_disk::<Vec<_>>(1)
            .into_iter()
            .find(|c| *c != cell)
            .expect("neighbor cell");
        
        let fare = calculate_trip_fare(cell, nearby);
        assert!(fare >= BASE_FARE, "fare should be at least base fare");
        
        // For a very short trip, fare should be close to base
        let distance = distance_km_between_cells(cell, nearby);
        let expected = BASE_FARE + (distance * PER_KM_RATE);
        assert!((fare - expected).abs() < 0.01, "fare calculation should match formula");
    }

    #[test]
    fn pricing_config_default() {
        let config = PricingConfig::default();
        assert_eq!(config.base_fare, BASE_FARE);
        assert_eq!(config.per_km_rate, PER_KM_RATE);
        assert_eq!(config.commission_rate, 0.0);
    }

    #[test]
    fn fare_with_custom_config() {
        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
        let nearby = cell
            .grid_disk::<Vec<_>>(1)
            .into_iter()
            .find(|c| *c != cell)
            .expect("neighbor cell");
        
        let config = PricingConfig {
            base_fare: 3.0,
            per_km_rate: 2.0,
            commission_rate: 0.0,
            surge_enabled: false,
            surge_radius_k: 1,
            surge_max_multiplier: 2.0,
        };
        let fare = calculate_trip_fare_with_config(cell, nearby, config);
        let distance = distance_km_between_cells(cell, nearby);
        let expected = 3.0 + (distance * 2.0);
        assert!((fare - expected).abs() < 0.01, "fare calculation should match formula");
    }

    #[test]
    fn commission_calculations() {
        let fare = 100.0;
        let commission_rate = 0.15;
        
        let commission = calculate_commission(fare, commission_rate);
        assert_eq!(commission, 15.0);
        
        let driver_earnings = calculate_driver_earnings(fare, commission_rate);
        assert_eq!(driver_earnings, 85.0);
        
        let platform_revenue = calculate_platform_revenue(fare, commission_rate);
        assert_eq!(platform_revenue, 15.0);
        
        // Verify driver earnings + commission = fare
        assert!((driver_earnings + commission - fare).abs() < 0.01);
    }

    #[test]
    fn zero_commission() {
        let fare = 100.0;
        let commission_rate = 0.0;
        
        let commission = calculate_commission(fare, commission_rate);
        assert_eq!(commission, 0.0);
        
        let driver_earnings = calculate_driver_earnings(fare, commission_rate);
        assert_eq!(driver_earnings, fare);
    }
}
