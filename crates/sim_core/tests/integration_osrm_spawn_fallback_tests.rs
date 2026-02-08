#![cfg(feature = "osrm")]

mod support;

use sim_core::routing::osrm_spawn::{OsrmSpawnClient, OsrmSpawnError};

#[test]
fn osrm_spawn_empty_trace_returns_no_match_without_network() {
    let client = OsrmSpawnClient::new("http://127.0.0.1:9");
    let result = client.snap_with_defaults(&[]);
    assert!(matches!(result, Err(OsrmSpawnError::NoMatch)));
}
