//! Simple pricing system for calculating trip fares.

use h3o::CellIndex;

use crate::spatial::distance_km_between_cells;

/// Base fare in currency units (e.g., dollars).
pub const BASE_FARE: f64 = 2.50;

/// Per-kilometer rate in currency units.
pub const PER_KM_RATE: f64 = 1.50;

/// Calculate fare for a trip based on distance.
/// 
/// Formula: `fare = BASE_FARE + (distance_km * PER_KM_RATE)`
/// 
/// Returns the driver earnings (fare amount). Commission can be deducted later if needed.
pub fn calculate_trip_fare(pickup: CellIndex, dropoff: CellIndex) -> f64 {
    let distance_km = distance_km_between_cells(pickup, dropoff);
    BASE_FARE + (distance_km * PER_KM_RATE)
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
}
