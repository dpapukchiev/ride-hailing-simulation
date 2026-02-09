use h3o::LatLng;

use super::error::OsrmSpawnError;
use super::response::{OsrmMatchResponse, OsrmNearestMatch, OsrmNearestResponse, OsrmSpawnMatch};
use super::selection::{first_tracepoint_index, select_best_matching};

pub(super) fn parse_match_response(
    resp: OsrmMatchResponse,
) -> Result<OsrmSpawnMatch, OsrmSpawnError> {
    if resp.code != "Ok" {
        return Err(OsrmSpawnError::Api(resp.code));
    }

    let matchings = resp.matchings.ok_or(OsrmSpawnError::NoMatch)?;
    let tracepoints = resp.tracepoints.unwrap_or_default();

    let (matching_idx, matching) =
        select_best_matching(&matchings, &tracepoints).ok_or(OsrmSpawnError::NoMatch)?;

    let tracepoint_idx =
        first_tracepoint_index(&tracepoints, matching_idx).ok_or(OsrmSpawnError::NoMatch)?;
    let tracepoint = tracepoints[tracepoint_idx]
        .as_ref()
        .ok_or(OsrmSpawnError::NoMatch)?;

    let snapped_coordinate = LatLng::new(tracepoint.location[1], tracepoint.location[0])
        .map_err(|_| OsrmSpawnError::NoMatch)?;

    let road_name = tracepoint
        .name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .or_else(|| matching.name.clone().filter(|name| !name.trim().is_empty()));

    Ok(OsrmSpawnMatch {
        coordinate: snapped_coordinate,
        confidence: matching.confidence,
        distance_m: matching.distance,
        road_name,
    })
}

pub(super) fn parse_nearest_response(
    resp: OsrmNearestResponse,
) -> Result<OsrmNearestMatch, OsrmSpawnError> {
    if resp.code != "Ok" {
        return Err(OsrmSpawnError::Api(resp.code));
    }

    let waypoint = resp.waypoints.first().ok_or(OsrmSpawnError::NoMatch)?;
    let snapped_coordinate = LatLng::new(waypoint.location[1], waypoint.location[0])
        .map_err(|_| OsrmSpawnError::NoMatch)?;

    let road_name = waypoint.name.clone().filter(|name| !name.trim().is_empty());

    Ok(OsrmNearestMatch {
        coordinate: snapped_coordinate,
        distance_m: waypoint.distance,
        road_name,
    })
}
