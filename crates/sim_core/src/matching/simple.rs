use bevy_ecs::prelude::Entity;
use h3o::CellIndex;

use super::algorithm::MatchingAlgorithm;

/// Simple matching algorithm that finds the first available driver within the match radius.
/// This preserves the original "first match wins" behavior.
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
                let dist = rider_pos
                    .grid_distance(*driver_pos)
                    .unwrap_or(i32::MAX);
                dist >= 0 && dist <= match_radius as i32
            })
            .map(|(driver_entity, _)| *driver_entity)
    }
}
