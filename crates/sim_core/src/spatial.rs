//! Spatial operations: H3-based geographic indexing and distance calculations.
//!
//! This module provides:
//!
//! - **GeoIndex**: Wrapper for H3 resolution configuration
//! - **Grid disk queries**: Find cells within K grid distance
//! - **Distance calculations**: Haversine distance between H3 cells
//!
//! Default resolution is 9 (~240m cell size), suitable for city-scale simulations.

use std::sync::{Mutex, OnceLock};
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

/// Global distance cache (10,000 entries, ~160KB memory).
fn get_distance_cache() -> &'static Mutex<LruCache<(CellIndex, CellIndex), f64>> {
    static CACHE: OnceLock<Mutex<LruCache<(CellIndex, CellIndex), f64>>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(LruCache::new(
            NonZeroUsize::new(10_000).expect("cache size must be non-zero")
        ))
    })
}

/// Calculate distance between two H3 cells with LRU caching.
/// 
/// Uses a global LRU cache to avoid repeated H3 cell → LatLng conversions
/// and Haversine calculations for frequently accessed cell pairs.
pub fn distance_km_between_cells(a: CellIndex, b: CellIndex) -> f64 {
    // Use symmetric key (smaller cell first) to maximize cache hits
    let key = if a < b { (a, b) } else { (b, a) };
    
    let mut cache = get_distance_cache().lock().unwrap();
    
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
