//! Spatial index update system: maintains H3 cell → entity mappings for efficient spatial queries.
//!
//! Updates the spatial index when entities are spawned, despawned, or move to new cells.

use bevy_ecs::prelude::{Added, Changed, Entity, Query, RemovedComponents, ResMut, With};

use crate::ecs::{Driver, Position, Rider};
use crate::spatial::SpatialIndex;

/// Updates the spatial index when riders are added, removed, or move.
/// Early returns if no changes detected to minimize overhead.
/// Only runs if SpatialIndex resource exists (optional for small scenarios).
pub fn update_spatial_index_riders_system(
    spatial_index: Option<ResMut<SpatialIndex>>,
    added_riders: Query<(Entity, &Position), Added<Rider>>,
    changed_positions: Query<(Entity, &Position), (Changed<Position>, With<Rider>)>,
    mut removed_riders: RemovedComponents<Rider>,
) {
    let Some(mut spatial_index) = spatial_index else {
        return; // Spatial index not enabled for this scenario
    };
    // Early return if no changes (avoid processing overhead when not needed)
    // Note: is_empty() is cheap for Changed/Added queries (just checks change tracking)
    let has_added = !added_riders.is_empty();
    let has_changed = !changed_positions.is_empty();
    let has_removed = !removed_riders.is_empty();

    if !has_added && !has_changed && !has_removed {
        return;
    }

    // Handle newly added riders
    if has_added {
        for (entity, position) in added_riders.iter() {
            spatial_index.insert_rider(entity, position.0);
        }
    }

    // Handle position changes
    if has_changed {
        for (entity, position) in changed_positions.iter() {
            if let Some(old_cell) = spatial_index.get_rider_cell(entity) {
                if old_cell != position.0 {
                    spatial_index.update_rider_position(entity, old_cell, position.0);
                }
            } else {
                // Entity not in index yet (shouldn't happen, but handle gracefully)
                spatial_index.insert_rider(entity, position.0);
            }
        }
    }

    // Handle removed riders
    if has_removed {
        for entity in removed_riders.read() {
            spatial_index.remove_rider(entity);
        }
    }
}

/// Updates the spatial index when drivers are added, removed, or move.
/// Early returns if no changes detected to minimize overhead.
/// Only runs if SpatialIndex resource exists (optional for small scenarios).
pub fn update_spatial_index_drivers_system(
    spatial_index: Option<ResMut<SpatialIndex>>,
    added_drivers: Query<(Entity, &Position), Added<Driver>>,
    changed_positions: Query<(Entity, &Position), (Changed<Position>, With<Driver>)>,
    mut removed_drivers: RemovedComponents<Driver>,
) {
    let Some(mut spatial_index) = spatial_index else {
        return; // Spatial index not enabled for this scenario
    };
    // Early return if no changes (avoid processing overhead when not needed)
    // Note: is_empty() is cheap for Changed/Added queries (just checks change tracking)
    let has_added = !added_drivers.is_empty();
    let has_changed = !changed_positions.is_empty();
    let has_removed = !removed_drivers.is_empty();

    if !has_added && !has_changed && !has_removed {
        return;
    }

    // Handle newly added drivers
    if has_added {
        for (entity, position) in added_drivers.iter() {
            spatial_index.insert_driver(entity, position.0);
        }
    }

    // Handle position changes
    if has_changed {
        for (entity, position) in changed_positions.iter() {
            if let Some(old_cell) = spatial_index.get_driver_cell(entity) {
                if old_cell != position.0 {
                    spatial_index.update_driver_position(entity, old_cell, position.0);
                }
            } else {
                // Entity not in index yet (shouldn't happen, but handle gracefully)
                spatial_index.insert_driver(entity, position.0);
            }
        }
    }

    // Handle removed drivers
    if has_removed {
        for entity in removed_drivers.read() {
            spatial_index.remove_driver(entity);
        }
    }
}
