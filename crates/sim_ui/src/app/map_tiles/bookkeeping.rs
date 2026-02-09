use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use super::types::{CachedProjection, TileGeometry, TileKey, TileResult};

pub(crate) fn current_inflight_limit(tile_count: usize) -> usize {
    const WARMUP_TILES: usize = 6;
    const WARMUP_LIMIT: usize = 4;
    const MAX_LIMIT: usize = 12;
    if tile_count >= WARMUP_TILES {
        MAX_LIMIT
    } else {
        WARMUP_LIMIT
    }
}

pub(crate) fn apply_tile_result(
    inflight: &mut HashSet<TileKey>,
    errors: &mut HashMap<TileKey, String>,
    result: TileResult,
) -> Option<(TileKey, TileGeometry)> {
    inflight.remove(&result.key);
    if let Some(error) = result.error {
        errors.insert(result.key, error);
        return None;
    }

    result.geometry.map(|geometry| (result.key, geometry))
}

pub(crate) fn evict_stale_projections(projection_cache: &mut HashMap<TileKey, CachedProjection>) {
    let now = Instant::now();
    let ttl = Duration::from_secs(5);
    projection_cache.retain(|_, entry| now.duration_since(entry.last_used) <= ttl);
}
