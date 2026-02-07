use bevy_ecs::prelude::Entity;
use h3o::CellIndex;

use super::algorithm::MatchingAlgorithm;

/// Simple matching algorithm: first match within radius.
///
/// This algorithm implements a "first-come-first-served" matching strategy.
/// It returns the first available driver found within the match radius, without
/// considering distance, ETA, or other optimization criteria.
///
/// # Algorithm Behavior
///
/// 1. Iterates through `available_drivers` in order
/// 2. For each driver, checks if they are within `match_radius` H3 grid distance
/// 3. Returns the first driver found within radius, or `None` if no match
///
/// # Use Cases
///
/// This algorithm is useful for:
/// - Baseline comparisons with more sophisticated algorithms
/// - Simulations where matching speed is more important than optimization
/// - Testing and debugging (deterministic, predictable behavior)
///
/// # Performance
///
/// Time complexity: O(n) where n is the number of available drivers.
/// This is the fastest matching algorithm but may not produce optimal results.
#[derive(Debug, Default)]
pub struct SimpleMatching;

impl MatchingAlgorithm for SimpleMatching {
    fn find_match(
        &self,
        _rider_entity: Entity,
        rider_pos: CellIndex,
        _rider_destination: Option<CellIndex>,
        available_drivers: &[(Entity, CellIndex)],
        match_radius: u32,
        _clock_now_ms: u64,
    ) -> Option<Entity> {
        available_drivers
            .iter()
            .find(|(_driver_entity, driver_pos)| {
                let dist = rider_pos.grid_distance(*driver_pos).unwrap_or(i32::MAX);
                dist >= 0 && dist <= match_radius as i32
            })
            .map(|(driver_entity, _)| *driver_entity)
    }
}
