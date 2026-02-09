use h3o::LatLng;
use reqwest::{blocking::Client, Url};
use std::time::Duration;

use super::error::OsrmSpawnError;
use super::parser::{parse_match_response, parse_nearest_response};
use super::radius::{default_radiuses, encode_radiuses};
use super::response::{OsrmMatchResponse, OsrmNearestMatch, OsrmNearestResponse, OsrmSpawnMatch};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

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

    /// Snap a single coordinate to the nearest road using OSRM `/nearest` as a fallback.
    pub fn snap_nearest(&self, point: LatLng) -> Result<OsrmNearestMatch, OsrmSpawnError> {
        let coord = format!("{:.6},{:.6}", point.lng(), point.lat());
        let url = Url::parse(&format!("{}/nearest/v1/driving/{}", self.endpoint, coord))
            .map_err(|err| OsrmSpawnError::Api(format!("failed to build OSRM URL: {}", err)))?;

        let response = self.client.get(url).send().map_err(OsrmSpawnError::Http)?;
        let parsed: OsrmNearestResponse = response.json().map_err(OsrmSpawnError::Json)?;
        parse_nearest_response(parsed)
    }

    /// Convenience helper that applies conservative radii (30m start, 15m rest).
    pub fn snap_with_defaults(&self, points: &[LatLng]) -> Result<OsrmSpawnMatch, OsrmSpawnError> {
        let radiuses = default_radiuses(points.len());
        self.snap_trace(points, &radiuses)
    }
}
