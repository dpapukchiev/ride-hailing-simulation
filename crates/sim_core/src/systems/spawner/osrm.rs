#[cfg(feature = "osrm")]
use h3o::LatLng;
#[cfg(feature = "osrm")]
use rand::Rng;

#[cfg(feature = "osrm")]
use crate::routing::osrm_spawn::{radiuses_for_attempt, OsrmSpawnClient, OsrmSpawnMatch};

#[cfg(feature = "osrm")]
use super::MaybeOsrmSpawnMetrics;

#[cfg(feature = "osrm")]
const MAX_OSRM_MATCH_ATTEMPTS: usize = 2;
#[cfg(feature = "osrm")]
const MIN_OSRM_MATCH_CONFIDENCE: f64 = 0.7;
#[cfg(feature = "osrm")]
const MAX_OSRM_MATCH_DISTANCE_M: f64 = 40.0;
#[cfg(feature = "osrm")]
const SPAWN_TRACE_JITTER_DEG: f64 = 0.00035;

#[cfg(feature = "osrm")]
pub(super) fn try_snap_spawn_location<R: Rng>(
    client: &OsrmSpawnClient,
    rng: &mut R,
    candidate: LatLng,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
    osrm_metrics: MaybeOsrmSpawnMetrics<'_>,
) -> Option<LatLng> {
    for attempt in 0..MAX_OSRM_MATCH_ATTEMPTS {
        let trace = build_spawn_trace(rng, candidate, lat_min, lat_max, lng_min, lng_max);
        if trace.is_empty() {
            break;
        }

        if let Some(metrics) = osrm_metrics {
            metrics.record_match_attempt();
        }
        let radiuses = radiuses_for_attempt(attempt, trace.len());
        let matching = match client.snap_trace(&trace, &radiuses) {
            Ok(matching) => matching,
            Err(_) => {
                if let Some(metrics) = osrm_metrics {
                    metrics.record_match_error();
                }
                continue;
            }
        };

        if !is_snap_acceptable(&matching, lat_min, lat_max, lng_min, lng_max, osrm_metrics) {
            continue;
        }

        if let Some(metrics) = osrm_metrics {
            metrics.record_match_success();
        }
        return Some(matching.coordinate);
    }

    if let Some(metrics) = osrm_metrics {
        metrics.record_nearest_attempt();
    }
    if let Ok(nearest) = client.snap_nearest(candidate) {
        if coordinate_within_bounds(&nearest.coordinate, lat_min, lat_max, lng_min, lng_max) {
            if let Some(metrics) = osrm_metrics {
                metrics.record_nearest_success();
            }
            return Some(nearest.coordinate);
        }
        if let Some(metrics) = osrm_metrics {
            metrics.record_nearest_rejected_oob();
        }
    } else if let Some(metrics) = osrm_metrics {
        metrics.record_nearest_failure();
    }

    None
}

#[cfg(feature = "osrm")]
fn is_snap_acceptable(
    snap: &OsrmSpawnMatch,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
    metrics: MaybeOsrmSpawnMetrics<'_>,
) -> bool {
    if snap.confidence < MIN_OSRM_MATCH_CONFIDENCE {
        if let Some(m) = metrics {
            m.record_match_rejected_confidence();
        }
        return false;
    }

    if snap
        .distance_m
        .map(|dist| dist > MAX_OSRM_MATCH_DISTANCE_M)
        .unwrap_or(false)
    {
        if let Some(m) = metrics {
            m.record_match_rejected_distance();
        }
        return false;
    }

    if !coordinate_within_bounds(&snap.coordinate, lat_min, lat_max, lng_min, lng_max) {
        if let Some(m) = metrics {
            m.record_match_rejected_oob();
        }
        return false;
    }

    true
}

#[cfg(feature = "osrm")]
fn build_spawn_trace<R: Rng>(
    rng: &mut R,
    center: LatLng,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
) -> Vec<LatLng> {
    let mut trace = Vec::with_capacity(3);
    trace.push(center);
    for _ in 0..2 {
        let lat = clamp_latlng(
            center.lat() + rng.gen_range(-SPAWN_TRACE_JITTER_DEG..=SPAWN_TRACE_JITTER_DEG),
            lat_min,
            lat_max,
            -90.0,
            90.0,
        );
        let lng = clamp_latlng(
            center.lng() + rng.gen_range(-SPAWN_TRACE_JITTER_DEG..=SPAWN_TRACE_JITTER_DEG),
            lng_min,
            lng_max,
            -180.0,
            180.0,
        );
        if let Ok(point) = LatLng::new(lat, lng) {
            trace.push(point);
        }
    }
    trace
}

#[cfg(feature = "osrm")]
fn clamp_latlng(
    value: f64,
    min_bound: f64,
    max_bound: f64,
    global_min: f64,
    global_max: f64,
) -> f64 {
    let bounded = value.clamp(global_min, global_max);
    bordered_clamp(bounded, min_bound, max_bound)
}

#[cfg(feature = "osrm")]
fn bordered_clamp(value: f64, min_bound: f64, max_bound: f64) -> f64 {
    let min = min_bound.min(max_bound);
    let max = min_bound.max(max_bound);
    value.clamp(min, max)
}

#[cfg(feature = "osrm")]
fn coordinate_within_bounds(
    location: &LatLng,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
) -> bool {
    let lat = location.lat();
    let lng = location.lng();
    lat >= lat_min && lat <= lat_max && lng >= lng_min && lng <= lng_max
}
