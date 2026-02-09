use std::collections::HashMap;
use std::time::Instant;

use super::types::{CachedProjection, ProjectionBounds, TileGeometry, TileKey};

pub(crate) fn cached_projection_lines<'a>(
    projection_cache: &'a mut HashMap<TileKey, CachedProjection>,
    key: &TileKey,
) -> Option<&'a [Vec<(f32, f32)>]> {
    projection_cache.get_mut(key).map(|entry| {
        entry.last_used = Instant::now();
        entry.normalized_lines.as_slice()
    })
}

pub(crate) fn cache_projection_from_geometry(
    projection_cache: &mut HashMap<TileKey, CachedProjection>,
    current_projection_bounds: Option<ProjectionBounds>,
    key: TileKey,
    geometry: &TileGeometry,
) {
    let bounds = match current_projection_bounds {
        Some(bounds) => bounds,
        None => return,
    };
    let lat_span = bounds.lat_span();
    let lng_span = bounds.lng_span();
    if lat_span <= 0.0 || lng_span <= 0.0 {
        return;
    }

    const TOLERANCE: f32 = 0.002;
    let mut normalized_lines = Vec::new();
    for line in &geometry.lines {
        let mut projected = Vec::new();
        let mut last_point: Option<(f32, f32)> = None;
        for &(lat, lng) in line {
            let mut x = ((lng - bounds.lng_min) / lng_span) as f32;
            let mut y = ((bounds.lat_max - lat) / lat_span) as f32;
            x = x.clamp(0.0, 1.0);
            y = y.clamp(0.0, 1.0);
            let point = (x, y);
            if let Some(last) = last_point {
                if (point.0 - last.0).abs() < TOLERANCE && (point.1 - last.1).abs() < TOLERANCE {
                    continue;
                }
            }
            projected.push(point);
            last_point = Some(point);
        }
        if projected.len() >= 2 {
            normalized_lines.push(projected);
        }
    }
    if normalized_lines.is_empty() {
        return;
    }
    projection_cache.insert(
        key,
        CachedProjection {
            normalized_lines,
            last_used: Instant::now(),
        },
    );
}

pub(crate) fn projection_bounds_from_signature(
    signature: super::types::MapSignature,
) -> Option<ProjectionBounds> {
    ProjectionBounds::new(
        signature.lat_min as f64 / 1_000_000.0,
        signature.lat_max as f64 / 1_000_000.0,
        signature.lng_min as f64 / 1_000_000.0,
        signature.lng_max as f64 / 1_000_000.0,
    )
}
