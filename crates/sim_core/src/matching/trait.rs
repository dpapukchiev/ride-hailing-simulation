use bevy_ecs::prelude::Entity;
use h3o::CellIndex;

use super::types::MatchResult;

/// Trait for matching algorithms that can find driver-rider pairings.
///
/// Matching algorithms determine which driver should be assigned to a waiting rider.
/// Different algorithms optimize for different objectives (e.g., distance, ETA, global efficiency).
///
/// # Examples
///
/// ```rust,no_run
/// use sim_core::matching::{MatchingAlgorithm, SimpleMatching};
/// use bevy_ecs::prelude::Entity;
/// use h3o::CellIndex;
///
/// let algorithm = SimpleMatching::default();
/// let driver = algorithm.find_match(
///     Entity::from_raw(1),
///     CellIndex::try_from(0x8a1fb46622dffff).unwrap(),
///     None,
///     &[(Entity::from_raw(2), CellIndex::try_from(0x8a1fb46622dffff).unwrap())],
///     5,
///     0,
/// );
/// ```
pub trait MatchingAlgorithm: Send + Sync {
    /// Find a match for a single rider.
    ///
    /// Searches through available drivers within the match radius and returns the best match
    /// according to the algorithm's scoring criteria.
    ///
    /// # Arguments
    ///
    /// * `rider_entity` - The entity ID of the rider requesting a match
    /// * `rider_pos` - The H3 cell position of the rider (pickup location)
    /// * `rider_destination` - Optional destination H3 cell (for trip-based optimization)
    /// * `available_drivers` - Slice of (driver_entity, driver_position) tuples for idle drivers
    /// * `match_radius` - Maximum H3 grid distance for matching (0 = same cell only)
    /// * `clock_now_ms` - Current simulation time in milliseconds (for time-based optimization)
    ///
    /// # Returns
    ///
    /// Returns `Some(driver_entity)` if a match is found, `None` otherwise.
    ///
    /// # Algorithm Behavior
    ///
    /// The algorithm should:
    /// 1. Filter drivers within `match_radius` H3 grid distance from `rider_pos`
    /// 2. Score each candidate driver according to the algorithm's criteria
    /// 3. Return the driver with the best score, or `None` if no drivers are within radius
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
    ///
    /// This method allows algorithms to optimize globally across all waiting riders,
    /// potentially achieving better overall matching quality than sequential single-rider matching.
    ///
    /// # Arguments
    ///
    /// * `riders` - Slice of (rider_entity, rider_position, rider_destination) tuples
    /// * `available_drivers` - Slice of (driver_entity, driver_position) tuples for idle drivers
    /// * `match_radius` - Maximum H3 grid distance for matching
    /// * `clock_now_ms` - Current simulation time in milliseconds
    ///
    /// # Returns
    ///
    /// Returns a vector of `MatchResult` containing successful matches. Riders without matches
    /// are excluded from the result.
    ///
    /// # Default Implementation
    ///
    /// The default implementation calls `find_match` sequentially for each rider.
    /// Algorithms can override this to implement global optimization (e.g., bipartite matching
    /// using the Hungarian algorithm for maximum-weight matching).
    ///
    /// # Performance Considerations
    ///
    /// For large numbers of riders and drivers, batch optimization can significantly improve
    /// matching quality but may have higher computational cost. Consider the trade-off based
    /// on simulation scale and requirements.
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
