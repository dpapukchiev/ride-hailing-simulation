//! Hungarian (Kuhn-Munkres) algorithm for maximum-weight bipartite matching.
//!
//! Uses the same scoring as CostBasedMatching (distance + ETA) but optimizes
//! globally across all rider-driver pairs in a batch to minimize total cost.

use bevy_ecs::prelude::Entity;
use h3o::CellIndex;
use pathfinding::kuhn_munkres::{kuhn_munkres, Weights};

use crate::spatial::distance_km_between_cells;

use super::algorithm::MatchingAlgorithm;
use super::types::MatchResult;
use super::CostBasedMatching;

/// Average speed for ETA estimation (km/h), same as CostBasedMatching.
const AVG_SPEED_KMH: f64 = 40.0;

/// Scale factor to convert f64 score to i64 for the assignment algorithm.
const SCALE: f64 = 1_000_000.0;

/// Weight for pairs outside match radius (never selected).
/// Must be worse than any feasible score but not so extreme that negating and summing
/// (e.g. in pathfinding's internal use of neg()) overflows i64.
const INFEASIBLE: i64 = -1_000_000_000_000_i64; // -1e12; feasible scores are typically -1e9..0

/// Simple matrix type implementing pathfinding's Weights for i64.
struct I64Weights(Vec<Vec<i64>>);

impl Weights<i64> for I64Weights {
    fn rows(&self) -> usize {
        self.0.len()
    }

    fn columns(&self) -> usize {
        self.0.first().map_or(0, |r| r.len())
    }

    fn at(&self, row: usize, col: usize) -> i64 {
        self.0[row][col]
    }

    fn neg(&self) -> Self {
        I64Weights(
            self.0
                .iter()
                .map(|r| r.iter().map(|&x| x.saturating_neg()).collect())
                .collect(),
        )
    }
}

/// Hungarian matching: global optimization via maximum-weight bipartite assignment.
///
/// Uses the same cost model as CostBasedMatching (distance + ETA weight) but
/// solves the assignment problem so that the total cost across all matches is
/// minimized (equivalently, total score maximized). Implements only batch
/// matching; single-rider matching delegates to CostBasedMatching.
#[derive(Debug)]
pub struct HungarianMatching {
    eta_weight: f64,
    fallback: CostBasedMatching,
}

impl HungarianMatching {
    /// Create a new Hungarian matching algorithm with the given ETA weight.
    pub fn new(eta_weight: f64) -> Self {
        Self {
            eta_weight,
            fallback: CostBasedMatching::new(eta_weight),
        }
    }

    fn estimate_pickup_eta_ms(&self, distance_km: f64) -> u64 {
        if distance_km <= 0.0 {
            return 1000;
        }
        let eta_hours = distance_km / AVG_SPEED_KMH;
        (eta_hours * 3600.0 * 1000.0).max(1000.0) as u64
    }

    fn score_pairing(&self, pickup_distance_km: f64, pickup_eta_ms: u64) -> f64 {
        -pickup_distance_km - (pickup_eta_ms as f64 / 1000.0) * self.eta_weight
    }

    /// Convert f64 score to i64 weight (scale and clamp to avoid overflow).
    fn score_to_weight(score: f64) -> i64 {
        let w = score * SCALE;
        if w >= i64::MAX as f64 {
            i64::MAX
        } else if w <= i64::MIN as f64 {
            i64::MIN
        } else {
            w as i64
        }
    }

    /// Greedy batch matching for small batches (O(n*m) instead of O(n³)).
    /// For each rider, finds the best available driver within radius.
    fn greedy_batch_matches(
        &self,
        riders: &[(Entity, CellIndex, Option<CellIndex>)],
        available_drivers: &[(Entity, CellIndex)],
        match_radius: u32,
    ) -> Vec<MatchResult> {
        let mut results = Vec::new();
        let mut used_drivers = std::collections::HashSet::new();
        
        for (rider_entity, rider_pos, _) in riders {
            let mut best_driver: Option<(Entity, f64)> = None;
            
            for (driver_entity, driver_pos) in available_drivers {
                if used_drivers.contains(driver_entity) {
                    continue;
                }
                
                // Check grid distance first (cheap)
                let grid_dist = rider_pos
                    .grid_distance(*driver_pos)
                    .unwrap_or(i32::MAX);
                if grid_dist < 0 || grid_dist > match_radius as i32 {
                    continue;
                }
                
                // Calculate score
                let distance_km = distance_km_between_cells(*rider_pos, *driver_pos);
                let eta_ms = self.estimate_pickup_eta_ms(distance_km);
                let score = self.score_pairing(distance_km, eta_ms);
                
                if best_driver.map_or(true, |(_, best_score)| score > best_score) {
                    best_driver = Some((*driver_entity, score));
                }
            }
            
            if let Some((driver_entity, _)) = best_driver {
                used_drivers.insert(driver_entity);
                results.push(MatchResult {
                    rider_entity: *rider_entity,
                    driver_entity,
                });
            }
        }
        
        results
    }
}

impl Default for HungarianMatching {
    fn default() -> Self {
        Self::new(super::DEFAULT_ETA_WEIGHT)
    }
}

impl MatchingAlgorithm for HungarianMatching {
    fn find_match(
        &self,
        rider_entity: Entity,
        rider_pos: CellIndex,
        rider_destination: Option<CellIndex>,
        available_drivers: &[(Entity, CellIndex)],
        match_radius: u32,
        clock_now_ms: u64,
    ) -> Option<Entity> {
        self.fallback.find_match(
            rider_entity,
            rider_pos,
            rider_destination,
            available_drivers,
            match_radius,
            clock_now_ms,
        )
    }

    fn find_batch_matches(
        &self,
        riders: &[(Entity, CellIndex, Option<CellIndex>)],
        available_drivers: &[(Entity, CellIndex)],
        match_radius: u32,
        _clock_now_ms: u64,
    ) -> Vec<MatchResult> {
        if riders.is_empty() || available_drivers.is_empty() {
            return Vec::new();
        }

        // Early termination: use greedy matching for very small batches
        // Hungarian algorithm O(n³) overhead not worth it for small batches
        if riders.len() <= 10 && available_drivers.len() <= 20 {
            return self.greedy_batch_matches(riders, available_drivers, match_radius);
        }

        // Kuhn-Munkres requires rows <= columns. So we use the smaller set as rows.
        let (rider_idx_to_entity, _driver_idx_to_entity) = if riders.len() <= available_drivers.len() {
            (true, false)
        } else {
            (false, true)
        };

        let (rows, cols) = if rider_idx_to_entity {
            (riders.len(), available_drivers.len())
        } else {
            (available_drivers.len(), riders.len())
        };

        // Pre-filter feasible pairs by grid_distance before expensive distance calculations
        // This avoids computing distance_km_between_cells for pairs outside match radius
        let mut feasible_pairs = Vec::new();
        
        if rider_idx_to_entity {
            for (i, (_, rider_pos, _)) in riders.iter().enumerate() {
                for (j, (_, driver_pos)) in available_drivers.iter().enumerate() {
                    let grid_dist = rider_pos
                        .grid_distance(*driver_pos)
                        .unwrap_or(i32::MAX);
                    if grid_dist >= 0 && grid_dist <= match_radius as i32 {
                        feasible_pairs.push((i, j, *rider_pos, *driver_pos));
                    }
                }
            }
        } else {
            for (j, (_, rider_pos, _)) in riders.iter().enumerate() {
                for (i, (_, driver_pos)) in available_drivers.iter().enumerate() {
                    let grid_dist = rider_pos
                        .grid_distance(*driver_pos)
                        .unwrap_or(i32::MAX);
                    if grid_dist >= 0 && grid_dist <= match_radius as i32 {
                        feasible_pairs.push((i, j, *rider_pos, *driver_pos));
                    }
                }
            }
        }

        // Build matrix only for feasible pairs (others remain INFEASIBLE)
        let mut matrix = vec![vec![INFEASIBLE; cols]; rows];
        
        for (i, j, rider_pos, driver_pos) in feasible_pairs {
            let distance_km = distance_km_between_cells(rider_pos, driver_pos);
            let eta_ms = self.estimate_pickup_eta_ms(distance_km);
            let score = self.score_pairing(distance_km, eta_ms);
            matrix[i][j] = Self::score_to_weight(score);
        }

        let weights = I64Weights(matrix);

        // Only run if at least one feasible pair exists (avoid panic on all INFEASIBLE)
        let has_feasible = (0..weights.rows())
            .any(|r| (0..weights.columns()).any(|c| weights.at(r, c) > INFEASIBLE));
        if !has_feasible {
            return Vec::new();
        }

        let (_total, assignments) = kuhn_munkres(&weights);

        let mut results = Vec::new();
        if rider_idx_to_entity {
            for (rider_idx, &driver_idx) in assignments.iter().enumerate() {
                if driver_idx < available_drivers.len()
                    && weights.at(rider_idx, driver_idx) > INFEASIBLE
                {
                    results.push(MatchResult {
                        rider_entity: riders[rider_idx].0,
                        driver_entity: available_drivers[driver_idx].0,
                    });
                }
            }
        } else {
            for (driver_idx, &rider_idx) in assignments.iter().enumerate() {
                if rider_idx < riders.len()
                    && weights.at(driver_idx, rider_idx) > INFEASIBLE
                {
                    results.push(MatchResult {
                        rider_entity: riders[rider_idx].0,
                        driver_entity: available_drivers[driver_idx].0,
                    });
                }
            }
        }
        results
    }
}
