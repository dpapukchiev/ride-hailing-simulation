//! Spatial operations: H3-based geographic indexing and distance calculations.
//!
//! This module provides:
//!
//! - **GeoIndex**: Wrapper for H3 resolution configuration
//! - **Grid disk queries**: Find cells within K grid distance
//! - **Distance calculations**: Haversine distance between H3 cells
//! - **SpatialIndex**: H3 cell → entity mappings for efficient spatial queries
//!
//! Default resolution is 9 (~240m cell size), suitable for city-scale simulations.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use bevy_ecs::prelude::{Entity, Resource};
use h3o::{CellIndex, Resolution};
use lru::LruCache;
use std::num::NonZeroUsize;

#[derive(Debug, Clone, Copy)]
pub struct GeoIndex {
    resolution: Resolution,
}

impl GeoIndex {
    pub fn new(resolution: Resolution) -> Self {
        Self { resolution }
    }

    pub fn resolution(&self) -> Resolution {
        self.resolution
    }

    pub fn grid_disk(&self, origin: CellIndex, k: u32) -> Vec<CellIndex> {
        debug_assert_eq!(
            origin.resolution(),
            self.resolution,
            "origin resolution must match GeoIndex resolution"
        );
        origin.grid_disk::<Vec<_>>(k)
    }
}

/// Uncached distance calculation (internal use).
fn distance_km_between_cells_uncached(a: CellIndex, b: CellIndex) -> f64 {
    let a: h3o::LatLng = a.into();
    let b: h3o::LatLng = b.into();
    let (lat1, lon1) = (a.lat().to_radians(), a.lng().to_radians());
    let (lat2, lon2) = (b.lat().to_radians(), b.lng().to_radians());
    let dlat = lat2 - lat1;
    let dlon = lon2 - lon1;
    let sin_dlat = (dlat * 0.5).sin();
    let sin_dlon = (dlon * 0.5).sin();
    let h = sin_dlat * sin_dlat + lat1.cos() * lat2.cos() * sin_dlon * sin_dlon;
    let c = 2.0 * h.sqrt().atan2((1.0 - h).sqrt());
    6371.0 * c
}

/// Global distance cache (50,000 entries, ~800KB memory).
fn get_distance_cache() -> &'static Mutex<LruCache<(CellIndex, CellIndex), f64>> {
    static CACHE: OnceLock<Mutex<LruCache<(CellIndex, CellIndex), f64>>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(LruCache::new(
            NonZeroUsize::new(50_000).expect("cache size must be non-zero")
        ))
    })
}

/// Grid disk cache for surge pricing and spatial queries.
struct GridDiskCache {
    cache: Mutex<LruCache<(CellIndex, u32), Vec<CellIndex>>>,
}

impl GridDiskCache {
    fn new() -> Self {
        Self {
            cache: Mutex::new(LruCache::new(
                NonZeroUsize::new(1_000).expect("cache size must be non-zero")
            )),
        }
    }

    fn get_or_compute(&self, origin: CellIndex, k: u32, geo: &GeoIndex) -> Vec<CellIndex> {
        let mut cache = match self.cache.lock() {
            Ok(guard) => guard,
            Err(_) => return geo.grid_disk(origin, k), // Fallback: compute without cache if mutex poisoned
        };
        cache.get_or_insert((origin, k), || geo.grid_disk(origin, k)).clone()
    }
}

static GRID_DISK_CACHE: OnceLock<GridDiskCache> = OnceLock::new();

fn get_grid_disk_cache() -> &'static GridDiskCache {
    GRID_DISK_CACHE.get_or_init(GridDiskCache::new)
}

/// Path cache for movement system.
/// Only caches successful paths; failures are not cached (will retry, which is fine).
struct PathCache {
    cache: Mutex<LruCache<(CellIndex, CellIndex), Vec<CellIndex>>>,
}

impl PathCache {
    fn new() -> Self {
        Self {
            cache: Mutex::new(LruCache::new(
                NonZeroUsize::new(5_000).expect("cache size must be non-zero")
            )),
        }
    }

    fn get_or_compute(&self, from: CellIndex, to: CellIndex) -> Option<Vec<CellIndex>> {
        let mut cache = match self.cache.lock() {
            Ok(guard) => guard,
            Err(_) => return Self::compute_path(from, to), // Fallback: compute without cache if mutex poisoned
        };
        let key = if from < to { (from, to) } else { (to, from) };
        
        // Check cache first
        if let Some(cached) = cache.get(&key) {
            return Some(cached.clone());
        }
        
        // Compute path
        let path_result = Self::compute_path(from, to);
        
        // Cache successful paths only
        if let Some(cells) = &path_result {
            cache.put(key, cells.clone());
        }
        
        path_result
    }

    /// Compute a grid path between two cells without caching.
    fn compute_path(from: CellIndex, to: CellIndex) -> Option<Vec<CellIndex>> {
        from.grid_path_cells(to)
            .ok()
            .and_then(|path| {
                let cells: Vec<CellIndex> = path.filter_map(|cell| cell.ok()).collect();
                if cells.is_empty() {
                    None
                } else {
                    Some(cells)
                }
            })
    }
}

static PATH_CACHE: OnceLock<PathCache> = OnceLock::new();

fn get_path_cache() -> &'static PathCache {
    PATH_CACHE.get_or_init(PathCache::new)
}

/// Spatial index for efficient entity lookups by H3 cell.
/// 
/// Maintains mappings from H3 cells to entities (riders and drivers) for O(1) spatial queries
/// instead of scanning all entities. Updated incrementally as entities move or change state.
#[derive(Debug, Resource, Default)]
pub struct SpatialIndex {
    /// Map from H3 cell to rider entities in that cell
    riders_by_cell: HashMap<CellIndex, Vec<Entity>>,
    /// Map from H3 cell to driver entities in that cell
    drivers_by_cell: HashMap<CellIndex, Vec<Entity>>,
    /// Reverse mapping: rider entity → current cell (for efficient updates)
    rider_entity_to_cell: HashMap<Entity, CellIndex>,
    /// Reverse mapping: driver entity → current cell (for efficient updates)
    driver_entity_to_cell: HashMap<Entity, CellIndex>,
}

impl SpatialIndex {
    /// Create a new empty spatial index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a rider entity at the given cell.
    pub fn insert_rider(&mut self, entity: Entity, cell: CellIndex) {
        self.riders_by_cell.entry(cell).or_insert_with(Vec::new).push(entity);
        self.rider_entity_to_cell.insert(entity, cell);
    }

    /// Insert a driver entity at the given cell.
    pub fn insert_driver(&mut self, entity: Entity, cell: CellIndex) {
        self.drivers_by_cell.entry(cell).or_insert_with(Vec::new).push(entity);
        self.driver_entity_to_cell.insert(entity, cell);
    }

    /// Remove a rider entity from the index.
    pub fn remove_rider(&mut self, entity: Entity) {
        if let Some(cell) = self.rider_entity_to_cell.remove(&entity) {
            if let Some(entities) = self.riders_by_cell.get_mut(&cell) {
                entities.retain(|&e| e != entity);
                if entities.is_empty() {
                    self.riders_by_cell.remove(&cell);
                }
            }
        }
    }

    /// Remove a driver entity from the index.
    pub fn remove_driver(&mut self, entity: Entity) {
        if let Some(cell) = self.driver_entity_to_cell.remove(&entity) {
            if let Some(entities) = self.drivers_by_cell.get_mut(&cell) {
                entities.retain(|&e| e != entity);
                if entities.is_empty() {
                    self.drivers_by_cell.remove(&cell);
                }
            }
        }
    }

    /// Update a rider's position (remove from old cell, add to new cell).
    pub fn update_rider_position(&mut self, entity: Entity, old_cell: CellIndex, new_cell: CellIndex) {
        if old_cell == new_cell {
            return;
        }
        // Remove from old cell
        if let Some(entities) = self.riders_by_cell.get_mut(&old_cell) {
            entities.retain(|&e| e != entity);
            if entities.is_empty() {
                self.riders_by_cell.remove(&old_cell);
            }
        }
        // Add to new cell
        self.riders_by_cell.entry(new_cell).or_insert_with(Vec::new).push(entity);
        self.rider_entity_to_cell.insert(entity, new_cell);
    }

    /// Update a driver's position (remove from old cell, add to new cell).
    pub fn update_driver_position(&mut self, entity: Entity, old_cell: CellIndex, new_cell: CellIndex) {
        if old_cell == new_cell {
            return;
        }
        // Remove from old cell
        if let Some(entities) = self.drivers_by_cell.get_mut(&old_cell) {
            entities.retain(|&e| e != entity);
            if entities.is_empty() {
                self.drivers_by_cell.remove(&old_cell);
            }
        }
        // Add to new cell
        self.drivers_by_cell.entry(new_cell).or_insert_with(Vec::new).push(entity);
        self.driver_entity_to_cell.insert(entity, new_cell);
    }

    /// Get all rider entities in the given cells.
    pub fn get_riders_in_cells(&self, cells: &[CellIndex]) -> Vec<Entity> {
        let mut result = Vec::new();
        for cell in cells {
            if let Some(entities) = self.riders_by_cell.get(cell) {
                result.extend(entities.iter().copied());
            }
        }
        result
    }

    /// Get all driver entities in the given cells.
    pub fn get_drivers_in_cells(&self, cells: &[CellIndex]) -> Vec<Entity> {
        let mut result = Vec::new();
        for cell in cells {
            if let Some(entities) = self.drivers_by_cell.get(cell) {
                result.extend(entities.iter().copied());
            }
        }
        result
    }

    /// Get the current cell for a rider entity.
    pub fn get_rider_cell(&self, entity: Entity) -> Option<CellIndex> {
        self.rider_entity_to_cell.get(&entity).copied()
    }

    /// Get the current cell for a driver entity.
    pub fn get_driver_cell(&self, entity: Entity) -> Option<CellIndex> {
        self.driver_entity_to_cell.get(&entity).copied()
    }

    /// Clear all entries (for reset scenarios).
    pub fn clear(&mut self) {
        self.riders_by_cell.clear();
        self.drivers_by_cell.clear();
        self.rider_entity_to_cell.clear();
        self.driver_entity_to_cell.clear();
    }
}

/// Calculate distance between two H3 cells with LRU caching.
/// 
/// Uses a global LRU cache to avoid repeated H3 cell → LatLng conversions
/// and Haversine calculations for frequently accessed cell pairs.
pub fn distance_km_between_cells(a: CellIndex, b: CellIndex) -> f64 {
    // Use symmetric key (smaller cell first) to maximize cache hits
    let key = if a < b { (a, b) } else { (b, a) };
    
    let mut cache = match get_distance_cache().lock() {
        Ok(guard) => guard,
        Err(_) => return distance_km_between_cells_uncached(key.0, key.1), // Fallback: compute without cache if mutex poisoned
    };
    
    // Try to get from cache, compute if missing
    *cache.get_or_insert(key, || distance_km_between_cells_uncached(key.0, key.1))
}

impl Default for GeoIndex {
    fn default() -> Self {
        Self {
            resolution: Resolution::Nine,
        }
    }
}

/// Get grid disk with caching.
pub fn grid_disk_cached(origin: CellIndex, k: u32) -> Vec<CellIndex> {
    let geo = GeoIndex::default();
    get_grid_disk_cache().get_or_compute(origin, k, &geo)
}

/// Get grid path with caching.
pub fn grid_path_cells_cached(from: CellIndex, to: CellIndex) -> Option<Vec<CellIndex>> {
    get_path_cache().get_or_compute(from, to)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_disk_returns_neighbors_within_k() {
        let geo = GeoIndex::new(Resolution::Ten);
        let origin = CellIndex::try_from(0x8a1fb46622dffff).expect("valid cell");
        let cells = geo.grid_disk(origin, 1);

        assert!(cells.contains(&origin));
        assert!(!cells.is_empty());
        for cell in cells {
            let distance = origin.grid_distance(cell).expect("grid distance");
            assert!(distance <= 1);
        }
    }
}
