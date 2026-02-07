//! Test helpers for common test setup and utilities.
//!
//! This module provides shared test utilities to reduce duplication across test files.

use bevy_ecs::prelude::World;
use h3o::CellIndex;

/// A standard test cell used across test files for consistency.
/// This is a valid H3 cell at resolution 9 in the San Francisco Bay Area.
pub const TEST_CELL: u64 = 0x8a1fb46622dffff;

/// Get the test cell as a `CellIndex`.
///
/// # Panics
///
/// Panics if the test cell constant is invalid (should never happen).
pub fn test_cell() -> CellIndex {
    CellIndex::try_from(TEST_CELL).expect("TEST_CELL should be a valid H3 cell")
}

/// Get a neighbor cell of the test cell for testing purposes.
///
/// # Panics
///
/// Panics if no neighbor can be found (should never happen with a valid test cell).
pub fn test_neighbor_cell() -> CellIndex {
    test_cell()
        .grid_disk::<Vec<_>>(1)
        .into_iter()
        .find(|c| *c != test_cell())
        .expect("test cell should have neighbors")
}

/// Get a distant cell from the test cell for testing trip destinations.
///
/// # Panics
///
/// Panics if no distant cell can be found (should never happen with a valid test cell).
pub fn test_distant_cell() -> CellIndex {
    test_cell()
        .grid_disk::<Vec<_>>(2)
        .into_iter()
        .find(|c| *c != test_cell() && *c != test_neighbor_cell())
        .expect("test cell should have distant neighbors")
}

/// Create a basic test world with essential resources.
///
/// This is a convenience function for tests that need a minimal world setup.
/// For more complex scenarios, use the full `build_scenario` function.
pub fn create_test_world() -> World {
    let mut world = World::new();
    world.insert_resource(crate::clock::SimulationClock::default());
    world.insert_resource(crate::telemetry::SimTelemetry::default());
    world.insert_resource(crate::telemetry::SimSnapshotConfig::default());
    world.insert_resource(crate::telemetry::SimSnapshots::default());
    world.insert_resource(crate::speed::SpeedModel::with_range(Some(1), 40.0, 40.0));
    world.insert_resource(crate::scenario::create_simple_matching());
    world.insert_resource(crate::scenario::MatchRadius(0));
    world.insert_resource(crate::scenario::RiderCancelConfig::default());
    world
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_is_valid() {
        let cell = test_cell();
        assert_eq!(cell, CellIndex::try_from(TEST_CELL).unwrap());
    }

    #[test]
    fn test_neighbor_is_different() {
        let cell = test_cell();
        let neighbor = test_neighbor_cell();
        assert_ne!(cell, neighbor);
    }

    #[test]
    fn test_distant_cell_is_different() {
        let cell = test_cell();
        let neighbor = test_neighbor_cell();
        let distant = test_distant_cell();
        assert_ne!(cell, distant);
        assert_ne!(neighbor, distant);
    }
}
