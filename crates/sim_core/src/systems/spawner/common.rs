use h3o::{CellIndex, LatLng};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::spatial::cell_in_bounds;
use crate::spawner::{random_cell_in_bounds, SpawnWeighting};

use super::{MaybeOsrmSpawnClient, MaybeOsrmSpawnMetrics};

pub(super) struct SpawnLocation {
    pub(super) cell: CellIndex,
    pub(super) geo: LatLng,
}

/// Create a seeded RNG for spawn operations.
/// Uses config seed + spawn count for deterministic but varied randomness.
pub(super) fn create_spawn_rng(seed: u64, spawn_count: usize) -> StdRng {
    let combined_seed = seed.wrapping_add(spawn_count as u64);
    StdRng::seed_from_u64(combined_seed)
}

/// Generate a spawn position within the given bounds.
/// Falls back to center of bounds if coordinate generation fails.
///
/// # Safety
///
/// The `expect` call for fallback coordinates is safe because:
/// - We compute the center of valid geographic bounds (lat/lng in valid ranges)
/// - The bounds are validated when spawners are created (San Francisco Bay Area defaults)
/// - Center coordinates of valid bounds are always valid lat/lng values
fn generate_spawn_position<R: Rng>(
    rng: &mut R,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
) -> CellIndex {
    random_cell_in_bounds(rng, lat_min, lat_max, lng_min, lng_max).unwrap_or_else(|_| {
        let lat = (lat_min + lat_max) / 2.0;
        let lng = (lng_min + lng_max) / 2.0;
        let coord = h3o::LatLng::new(lat, lng)
            .expect("fallback coordinates should be valid (center of valid bounds)");
        coord.to_cell(h3o::Resolution::Nine)
    })
}

pub(super) fn bounded_weighted_spawn_cell<R, F>(
    weighting: Option<&SpawnWeighting>,
    rng: &mut R,
    sampler: F,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
) -> Option<CellIndex>
where
    R: Rng,
    F: Fn(&SpawnWeighting, &mut R) -> Option<CellIndex>,
{
    weighting
        .and_then(|w| sampler(w, rng))
        .filter(|cell| cell_in_bounds(*cell, lat_min, lat_max, lng_min, lng_max))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_spawn_location<R, F>(
    rng: &mut R,
    weighting: Option<&SpawnWeighting>,
    sampler: F,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
    _osrm_client: MaybeOsrmSpawnClient<'_>,
    _osrm_metrics: MaybeOsrmSpawnMetrics<'_>,
) -> SpawnLocation
where
    R: Rng,
    F: Fn(&SpawnWeighting, &mut R) -> Option<CellIndex>,
{
    let cell =
        bounded_weighted_spawn_cell(weighting, rng, sampler, lat_min, lat_max, lng_min, lng_max)
            .unwrap_or_else(|| generate_spawn_position(rng, lat_min, lat_max, lng_min, lng_max));

    let base_spawn_location = SpawnLocation {
        cell,
        geo: cell.into(),
    };
    #[cfg(feature = "osrm")]
    let mut spawn_location = base_spawn_location;
    #[cfg(not(feature = "osrm"))]
    let spawn_location = base_spawn_location;

    #[cfg(feature = "osrm")]
    if let Some(client) = _osrm_client {
        if let Some(snapped) = super::osrm::try_snap_spawn_location(
            client,
            rng,
            spawn_location.geo,
            lat_min,
            lat_max,
            lng_min,
            lng_max,
            _osrm_metrics,
        ) {
            spawn_location.geo = snapped;
            spawn_location.cell = snapped.to_cell(h3o::Resolution::Nine);
        }
    }

    spawn_location
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_weighted_spawn_cell_rejects_out_of_bounds() {
        let weighting = SpawnWeighting::berlin_hotspots();
        let mut rng = StdRng::seed_from_u64(0);
        let result = bounded_weighted_spawn_cell(
            Some(&weighting),
            &mut rng,
            |w, rng| w.sample_rider_cell(rng),
            0.0,
            1.0,
            0.0,
            1.0,
        );
        assert!(result.is_none());
    }

    #[test]
    fn bounded_weighted_spawn_cell_allows_in_bounds() {
        let weighting = SpawnWeighting::berlin_hotspots();
        let mut rng = StdRng::seed_from_u64(0);
        let result = bounded_weighted_spawn_cell(
            Some(&weighting),
            &mut rng,
            |w, rng| w.sample_rider_cell(rng),
            52.2,
            52.6,
            13.0,
            13.6,
        );
        assert!(result.is_some());
    }
}
