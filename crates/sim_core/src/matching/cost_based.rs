use bevy_ecs::prelude::Entity;
use h3o::CellIndex;

use crate::spatial::distance_km_between_cells;

use super::algorithm::MatchingAlgorithm;

/// Average speed for ETA estimation (km/h).
const AVG_SPEED_KMH: f64 = 40.0;

/// Cost-based matching algorithm that scores driver-rider pairings by distance and ETA.
/// Selects the driver with the best (lowest cost) score.
#[derive(Debug)]
pub struct CostBasedMatching {
    /// Weight for ETA in the scoring function. Higher values prioritize lower ETA more.
    pub eta_weight: f64,
}

impl CostBasedMatching {
    /// Create a new cost-based matching algorithm with the given ETA weight.
    pub fn new(eta_weight: f64) -> Self {
        Self { eta_weight }
    }

    /// Estimate pickup ETA in milliseconds based on distance.
    fn estimate_pickup_eta_ms(&self, distance_km: f64) -> u64 {
        if distance_km <= 0.0 {
            return 1000; // Minimum 1 second
        }
        let eta_hours = distance_km / AVG_SPEED_KMH;
        (eta_hours * 3600.0 * 1000.0).max(1000.0) as u64
    }

    /// Calculate a score for a driver-rider pairing. Lower cost = higher score.
    fn score_pairing(&self, pickup_distance_km: f64, pickup_eta_ms: u64) -> f64 {
        // Negative because lower distance/ETA is better, but we want higher scores
        -pickup_distance_km - (pickup_eta_ms as f64 / 1000.0) * self.eta_weight
    }
}

impl Default for CostBasedMatching {
    fn default() -> Self {
        Self::new(0.1)
    }
}

impl MatchingAlgorithm for CostBasedMatching {
    fn find_match(
        &self,
        _rider_entity: Entity,
        rider_pos: CellIndex,
        _rider_destination: Option<CellIndex>,
        available_drivers: &[(Entity, CellIndex)],
        match_radius: u32,
        _clock_now_ms: u64,
    ) -> Option<Entity> {
        let mut best_match: Option<(Entity, f64)> = None;

        for (driver_entity, driver_pos) in available_drivers {
            let grid_dist = rider_pos
                .grid_distance(*driver_pos)
                .unwrap_or(i32::MAX);

            // Filter by match radius
            if grid_dist < 0 || grid_dist > match_radius as i32 {
                continue;
            }

            let pickup_distance_km = distance_km_between_cells(rider_pos, *driver_pos);
            let pickup_eta_ms = self.estimate_pickup_eta_ms(pickup_distance_km);
            let score = self.score_pairing(pickup_distance_km, pickup_eta_ms);

            match best_match {
                None => best_match = Some((*driver_entity, score)),
                Some((_, best_score)) if score > best_score => {
                    best_match = Some((*driver_entity, score))
                }
                _ => {}
            }
        }

        best_match.map(|(driver_entity, _)| driver_entity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_closer_driver() {
        let matcher = CostBasedMatching::new(0.1);
        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
        
        // Find a nearby cell (grid distance 1)
        let nearby = cell
            .grid_disk::<Vec<_>>(1)
            .into_iter()
            .find(|c| *c != cell)
            .expect("neighbor cell");
        
        // Find a far cell (grid distance 3) that's actually farther in km
        // We need to ensure it's farther in haversine distance, not just grid distance
        let nearby_dist_km = distance_km_between_cells(cell, nearby);
        let far = cell
            .grid_disk::<Vec<_>>(3)
            .into_iter()
            .find(|c| {
                if *c == cell || *c == nearby {
                    return false;
                }
                let dist_km = distance_km_between_cells(cell, *c);
                dist_km > nearby_dist_km * 1.5 // Ensure it's significantly farther
            })
            .expect("distant cell");

        let rider_entity = bevy_ecs::prelude::Entity::from_raw(1);
        let nearby_driver = (bevy_ecs::prelude::Entity::from_raw(2), nearby);
        let far_driver = (bevy_ecs::prelude::Entity::from_raw(3), far);

        // Verify nearby is actually closer in km
        let far_dist_km = distance_km_between_cells(cell, far);
        assert!(nearby_dist_km < far_dist_km, "nearby driver should be closer: nearby={}, far={}", nearby_dist_km, far_dist_km);

        // Test with far driver first to ensure we select the better one
        let drivers = vec![far_driver, nearby_driver];
        let result = matcher.find_match(rider_entity, cell, None, &drivers, 5, 0);

        // Should select the closer driver (entity 2)
        assert_eq!(result, Some(bevy_ecs::prelude::Entity::from_raw(2)), 
                   "Expected nearby driver (entity 2), got {:?}. Nearby dist: {}km, Far dist: {}km", 
                   result, nearby_dist_km, far_dist_km);
    }
}
