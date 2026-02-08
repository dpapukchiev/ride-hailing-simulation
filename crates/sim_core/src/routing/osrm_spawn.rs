//! Helpers for snapping spawn candidates to roads via OSRM's `/match` service.
//!
//! This module wraps a blocking HTTP client for OSRM and exposes a deterministic
//! selection strategy so the simulation can land riders and drivers on drivable
//! streets without leaking details of the HTTP response.

use h3o::LatLng;
use reqwest::{blocking::Client, Url};
use serde::Deserialize;
use std::cmp::Ordering;
use std::time::Duration;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
const DEFAULT_FIRST_RADIUS_M: f64 = 30.0;
const DEFAULT_ADDITIONAL_RADIUS_M: f64 = 15.0;

/// Thin HTTP client for OSRM match-based snapping.
#[derive(Debug, Clone)]
pub struct OsrmSpawnClient {
    client: Client,
    endpoint: String,
}

impl OsrmSpawnClient {
    /// Create a client for the given OSRM endpoint (e.g. `http://localhost:5000`).
    pub fn new(endpoint: &str) -> Self {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("failed to build OSRM client");
        Self {
            client,
            endpoint: endpoint.trim_end_matches('/').to_string(),
        }
    }

    /// Snap the supplied points to roads and return the best match record.
    pub fn snap_trace(
        &self,
        points: &[LatLng],
        radiuses_m: &[f64],
    ) -> Result<OsrmSpawnMatch, OsrmSpawnError> {
        if points.is_empty() {
            return Err(OsrmSpawnError::NoMatch);
        }

        let coord_segment = points
            .iter()
            .map(|point| format!("{},{}", point.lng(), point.lat()))
            .collect::<Vec<_>>()
            .join(";");

        let base = format!("{}/match/v1/driving/{}", self.endpoint, coord_segment);
        let mut url = Url::parse(&base)
            .map_err(|err| OsrmSpawnError::Api(format!("failed to build OSRM URL: {}", err)))?;

        let radiuses = encode_radiuses(points.len(), radiuses_m);
        url.query_pairs_mut()
            .append_pair("gaps", "ignore")
            .append_pair("tidy", "true")
            .append_pair("geometries", "geojson")
            .append_pair("radiuses", &radiuses);

        let response = self.client.get(url).send().map_err(OsrmSpawnError::Http)?;

        let parsed: OsrmMatchResponse = response.json().map_err(OsrmSpawnError::Json)?;
        parse_match_response(parsed)
    }

    /// Convenience helper that applies conservative radii (30m start, 15m rest).
    pub fn snap_with_defaults(&self, points: &[LatLng]) -> Result<OsrmSpawnMatch, OsrmSpawnError> {
        let radiuses = default_radiuses(points.len());
        self.snap_trace(points, &radiuses)
    }
}

/// Result of a successful OSRM match snap.
#[derive(Clone, Debug, PartialEq)]
pub struct OsrmSpawnMatch {
    pub coordinate: LatLng,
    pub confidence: f64,
    pub distance_m: Option<f64>,
    pub road_name: Option<String>,
}

/// Errors encountered while snapping a candidate trace.
#[derive(Debug)]
pub enum OsrmSpawnError {
    Http(reqwest::Error),
    Json(reqwest::Error),
    Api(String),
    NoMatch,
}

impl From<reqwest::Error> for OsrmSpawnError {
    fn from(err: reqwest::Error) -> Self {
        OsrmSpawnError::Http(err)
    }
}

fn encode_radiuses(count: usize, radiuses: &[f64]) -> String {
    let default = radiuses
        .last()
        .copied()
        .unwrap_or(DEFAULT_ADDITIONAL_RADIUS_M)
        .max(0.0);

    (0..count)
        .map(|idx| {
            let radius = radiuses.get(idx).copied().unwrap_or(default).max(0.0);
            format!("{:.1}", radius)
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn default_radiuses(count: usize) -> Vec<f64> {
    if count == 0 {
        return Vec::new();
    }
    let mut radiuses = Vec::with_capacity(count);
    radiuses.push(DEFAULT_FIRST_RADIUS_M);
    radiuses.extend(std::iter::repeat_n(DEFAULT_ADDITIONAL_RADIUS_M, count - 1));
    radiuses
}

#[derive(Deserialize)]
struct OsrmMatchResponse {
    code: String,
    matchings: Option<Vec<OsrmMatching>>,
    tracepoints: Option<Vec<Option<OsrmTracepoint>>>,
}

#[derive(Deserialize)]
struct OsrmMatching {
    confidence: f64,
    distance: Option<f64>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct OsrmTracepoint {
    location: [f64; 2],
    name: Option<String>,
    #[serde(rename = "matchings_index")]
    matching_index: Option<usize>,
}

fn parse_match_response(resp: OsrmMatchResponse) -> Result<OsrmSpawnMatch, OsrmSpawnError> {
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

fn select_best_matching<'a>(
    matchings: &'a [OsrmMatching],
    tracepoints: &[Option<OsrmTracepoint>],
) -> Option<(usize, &'a OsrmMatching)> {
    let mut best: Option<(usize, &OsrmMatching)> = None;

    for (idx, matching) in matchings.iter().enumerate() {
        best = Some(if let Some((best_idx, best_matching)) = best {
            match matching
                .confidence
                .partial_cmp(&best_matching.confidence)
                .unwrap_or(Ordering::Equal)
            {
                Ordering::Greater => (idx, matching),
                Ordering::Less => (best_idx, best_matching),
                Ordering::Equal => {
                    let best_tp =
                        first_tracepoint_index(tracepoints, best_idx).unwrap_or(usize::MAX);
                    let next_tp = first_tracepoint_index(tracepoints, idx).unwrap_or(usize::MAX);
                    if next_tp < best_tp {
                        (idx, matching)
                    } else {
                        (best_idx, best_matching)
                    }
                }
            }
        } else {
            (idx, matching)
        });
    }

    best
}

fn first_tracepoint_index(
    tracepoints: &[Option<OsrmTracepoint>],
    matching_index: usize,
) -> Option<usize> {
    tracepoints
        .iter()
        .enumerate()
        .find(|(_, trace)| trace.as_ref().and_then(|tp| tp.matching_index) == Some(matching_index))
        .map(|(idx, _)| idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_radiuses_reuses_last_value_when_shorter() {
        let encoded = encode_radiuses(3, &[10.0, 20.0]);
        assert_eq!(encoded, "10.0;20.0;20.0");
    }

    #[test]
    fn select_best_matching_prefers_higher_confidence() {
        let tracepoints = vec![Some(OsrmTracepoint {
            location: [13.0, 52.0],
            name: None,
            matching_index: Some(1),
        })];
        let matchings = vec![
            OsrmMatching {
                confidence: 0.5,
                distance: None,
                name: None,
            },
            OsrmMatching {
                confidence: 0.8,
                distance: None,
                name: None,
            },
        ];

        let best = select_best_matching(&matchings, &tracepoints);
        assert_eq!(best.map(|(idx, _)| idx), Some(1));
    }

    #[test]
    fn select_best_matching_breaks_ties_by_tracepoint_order() {
        let tracepoints = vec![
            Some(OsrmTracepoint {
                location: [13.0, 52.0],
                name: None,
                matching_index: Some(1),
            }),
            Some(OsrmTracepoint {
                location: [14.0, 52.0],
                name: None,
                matching_index: Some(0),
            }),
        ];
        let matchings = vec![
            OsrmMatching {
                confidence: 0.7,
                distance: None,
                name: None,
            },
            OsrmMatching {
                confidence: 0.7,
                distance: None,
                name: None,
            },
        ];

        let best = select_best_matching(&matchings, &tracepoints);
        assert_eq!(best.map(|(idx, _)| idx), Some(1));
    }

    #[test]
    fn parse_match_response_returns_snap() {
        let response = OsrmMatchResponse {
            code: "Ok".to_string(),
            matchings: Some(vec![OsrmMatching {
                confidence: 0.9,
                distance: Some(25.0),
                name: Some("Sample Rd".to_string()),
            }]),
            tracepoints: Some(vec![Some(OsrmTracepoint {
                location: [13.0, 52.5],
                name: Some("Sample Rd".to_string()),
                matching_index: Some(0),
            })]),
        };

        let snap = parse_match_response(response).expect("should parse");
        assert_eq!(snap.confidence, 0.9);
        assert_eq!(snap.distance_m, Some(25.0));
        assert_eq!(snap.road_name.as_deref(), Some("Sample Rd"));
        assert_eq!(snap.coordinate, LatLng::new(52.5, 13.0).unwrap());
    }
}
