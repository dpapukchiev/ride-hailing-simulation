use bevy_ecs::prelude::Entity;
use h3o::CellIndex;

use super::types::MatchResult;

/// Trait for matching algorithms that can find driver-rider pairings.
pub trait MatchingAlgorithm: Send + Sync {
    /// Find a match for a single rider.
    /// Returns the best driver entity for the rider, or None if no match found.
    fn find_match(
        &self,
        rider_entity: Entity,
        rider_pos: CellIndex,
        rider_destination: Option<CellIndex>,
        available_drivers: &[(Entity, CellIndex)],
        match_radius: u32,
        clock_now_ms: u64,
    ) -> Option<Entity>;

    /// Find matches for multiple riders (batch optimization).
    /// Returns a vector of matches. Default implementation calls find_match for each rider.
    /// Algorithms can override this to optimize globally (e.g., bipartite matching).
    fn find_batch_matches(
        &self,
        riders: &[(Entity, CellIndex, Option<CellIndex>)],
        available_drivers: &[(Entity, CellIndex)],
        match_radius: u32,
        clock_now_ms: u64,
    ) -> Vec<MatchResult> {
        // Default: sequential matching
        riders
            .iter()
            .filter_map(|(rider_entity, rider_pos, rider_dest)| {
                self.find_match(
                    *rider_entity,
                    *rider_pos,
                    *rider_dest,
                    available_drivers,
                    match_radius,
                    clock_now_ms,
                )
                .map(|driver_entity| MatchResult {
                    rider_entity: *rider_entity,
                    driver_entity,
                })
            })
            .collect()
    }
}
