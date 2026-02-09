use h3o::LatLng;

/// Result of a successful OSRM match snap.
#[derive(Clone, Debug, PartialEq)]
pub struct OsrmSpawnMatch {
    pub coordinate: LatLng,
    pub confidence: f64,
    pub distance_m: Option<f64>,
    pub road_name: Option<String>,
}

/// Result of a nearest-neighbor snap.
#[derive(Clone, Debug, PartialEq)]
pub struct OsrmNearestMatch {
    pub coordinate: LatLng,
    pub distance_m: Option<f64>,
    pub road_name: Option<String>,
}

#[derive(serde::Deserialize)]
pub(super) struct OsrmMatchResponse {
    pub(super) code: String,
    pub(super) matchings: Option<Vec<OsrmMatching>>,
    pub(super) tracepoints: Option<Vec<Option<OsrmTracepoint>>>,
}

#[derive(serde::Deserialize)]
pub(super) struct OsrmMatching {
    pub(super) confidence: f64,
    pub(super) distance: Option<f64>,
    pub(super) name: Option<String>,
}

#[derive(serde::Deserialize)]
pub(super) struct OsrmTracepoint {
    pub(super) location: [f64; 2],
    pub(super) name: Option<String>,
    #[serde(rename = "matchings_index")]
    pub(super) matching_index: Option<usize>,
}

#[derive(serde::Deserialize)]
pub(super) struct OsrmNearestResponse {
    pub(super) code: String,
    pub(super) waypoints: Vec<OsrmNearestWaypoint>,
}

#[derive(serde::Deserialize)]
pub(super) struct OsrmNearestWaypoint {
    pub(super) location: [f64; 2],
    pub(super) name: Option<String>,
    pub(super) distance: Option<f64>,
}
