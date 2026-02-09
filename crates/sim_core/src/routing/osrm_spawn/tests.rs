use h3o::LatLng;

use super::parser::{parse_match_response, parse_nearest_response};
use super::radius::{encode_radiuses, fallback_radiuses};
use super::response::{
    OsrmMatchResponse, OsrmMatching, OsrmNearestResponse, OsrmNearestWaypoint, OsrmTracepoint,
};
use super::selection::select_best_matching;

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

#[test]
fn fallback_radiuses_repeat_last_value_when_shorter() {
    let radiuses = fallback_radiuses(3);
    assert_eq!(radiuses, vec![45.0, 30.0, 30.0]);
}

#[test]
fn parse_nearest_response_returns_snap() {
    let response = OsrmNearestResponse {
        code: "Ok".to_string(),
        waypoints: vec![OsrmNearestWaypoint {
            location: [13.0, 52.5],
            name: Some("Sample Rd".to_string()),
            distance: Some(12.0),
        }],
    };

    let snap = parse_nearest_response(response).expect("should parse");
    assert_eq!(snap.distance_m, Some(12.0));
    assert_eq!(snap.road_name.as_deref(), Some("Sample Rd"));
    assert_eq!(snap.coordinate, LatLng::new(52.5, 13.0).unwrap());
}
