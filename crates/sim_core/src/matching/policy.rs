use std::collections::HashMap;

use bevy_ecs::prelude::Entity;
use h3o::CellIndex;

use crate::spatial::distance_km_between_cells;

const AVG_SPEED_KMH: f64 = 40.0;

#[derive(Debug, Clone)]
pub struct MatchingScoreComponents {
    pub pickup_time_cost: f64,
    pub reposition_cost: f64,
    pub imbalance_penalty: f64,
    pub hotspot_bonus: f64,
}

impl MatchingScoreComponents {
    pub fn total(&self) -> f64 {
        self.pickup_time_cost + self.reposition_cost + self.imbalance_penalty - self.hotspot_bonus
    }
}

pub fn estimate_pickup_eta_ms(distance_km: f64) -> u64 {
    if distance_km <= 0.0 {
        return 1_000;
    }
    ((distance_km / AVG_SPEED_KMH) * 3_600_000.0).max(1_000.0) as u64
}

pub fn build_zone_counts(cells: impl Iterator<Item = CellIndex>) -> HashMap<CellIndex, usize> {
    let mut counts = HashMap::new();
    for cell in cells {
        *counts.entry(cell).or_insert(0) += 1;
    }
    counts
}

pub fn score_driver_for_rider(
    rider_pos: CellIndex,
    driver_pos: CellIndex,
    idle_zone_supply: &HashMap<CellIndex, usize>,
    waiting_zone_demand: &HashMap<CellIndex, usize>,
    target_idle_by_zone: &HashMap<CellIndex, usize>,
    minimum_zone_reserve: usize,
    hotspot_weight: f64,
) -> MatchingScoreComponents {
    let pickup_distance_km = distance_km_between_cells(rider_pos, driver_pos);
    let pickup_time_cost = estimate_pickup_eta_ms(pickup_distance_km) as f64 / 1_000.0;
    let reposition_cost = pickup_distance_km * 0.15;

    let source_supply = *idle_zone_supply.get(&driver_pos).unwrap_or(&0);
    let source_target = *target_idle_by_zone
        .get(&driver_pos)
        .unwrap_or(&minimum_zone_reserve);
    let would_drop_below_reserve = source_supply <= minimum_zone_reserve;
    let post_assign_supply = source_supply.saturating_sub(1);
    let imbalance_penalty = if would_drop_below_reserve {
        3_000.0
    } else if post_assign_supply < source_target {
        ((source_target - post_assign_supply) as f64) * 20.0
    } else {
        0.0
    };

    let hotspot_bonus =
        (*waiting_zone_demand.get(&rider_pos).unwrap_or(&0) as f64) * hotspot_weight;

    MatchingScoreComponents {
        pickup_time_cost,
        reposition_cost,
        imbalance_penalty,
        hotspot_bonus,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn choose_best_driver(
    rider_entity: Entity,
    rider_pos: CellIndex,
    available_drivers: &[(Entity, CellIndex)],
    match_radius: u32,
    idle_zone_supply: &HashMap<CellIndex, usize>,
    waiting_zone_demand: &HashMap<CellIndex, usize>,
    target_idle_by_zone: &HashMap<CellIndex, usize>,
    minimum_zone_reserve: usize,
    hotspot_weight: f64,
) -> Option<Entity> {
    let _ = rider_entity;
    available_drivers
        .iter()
        .filter(|(_, driver_pos)| {
            rider_pos
                .grid_distance(*driver_pos)
                .is_ok_and(|dist| dist >= 0 && dist <= match_radius as i32)
        })
        .min_by(|(_, a_pos), (_, b_pos)| {
            let a = score_driver_for_rider(
                rider_pos,
                *a_pos,
                idle_zone_supply,
                waiting_zone_demand,
                target_idle_by_zone,
                minimum_zone_reserve,
                hotspot_weight,
            )
            .total();
            let b = score_driver_for_rider(
                rider_pos,
                *b_pos,
                idle_zone_supply,
                waiting_zone_demand,
                target_idle_by_zone,
                minimum_zone_reserve,
                hotspot_weight,
            )
            .total();
            a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(entity, _)| *entity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_penalizes_depleting_reserve_zone() {
        let rider = CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
        let driver = rider;
        let mut supply = HashMap::new();
        supply.insert(driver, 1);
        let demand = HashMap::new();
        let target = supply.clone();

        let score = score_driver_for_rider(rider, driver, &supply, &demand, &target, 1, 0.35);
        assert!(score.imbalance_penalty >= 3000.0);
    }
}
