//! Route table export tool.
//!
//! Pre-computes routes for a grid of origin-destination cell pairs within
//! Berlin bounds by calling the H3GridRouteProvider (or OSRM when available),
//! then serializes the results to a JSON file that can be loaded for analysis.
//!
//! This example demonstrates the route export concept. For full OSRM-based
//! pre-computation, enable the `osrm` and `precomputed` features on `sim_core`:
//!
//! ```sh
//! cargo run --example route_export -p sim_experiments
//! ```
//!
//! With OSRM features:
//! ```sh
//! cargo run --example route_export -p sim_experiments \
//!     --features sim_core/osrm,sim_core/precomputed
//! ```

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use sim_core::routing::{H3GridRouteProvider, RouteProvider};

/// Berlin bounding box.
const LAT_MIN: f64 = 52.34;
const LAT_MAX: f64 = 52.68;
const LNG_MIN: f64 = 13.08;
const LNG_MAX: f64 = 13.76;

/// Sample N random H3 cells within Berlin bounds.
fn sample_cells(count: usize, seed: u64) -> Vec<h3o::CellIndex> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut cells = Vec::with_capacity(count);
    let mut attempts = 0;

    while cells.len() < count && attempts < count * 100 {
        attempts += 1;
        let lat = rng.gen_range(LAT_MIN..LAT_MAX);
        let lng = rng.gen_range(LNG_MIN..LNG_MAX);
        if let Ok(ll) = h3o::LatLng::new(lat, lng) {
            let cell = ll.to_cell(h3o::Resolution::Nine);
            if !cells.contains(&cell) {
                cells.push(cell);
            }
        }
    }

    cells
}

fn main() {
    let sample_count: usize = std::env::args()
        .skip_while(|a| a != "--sample-count")
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    let output_path = std::env::args()
        .skip_while(|a| a != "--output")
        .nth(1)
        .unwrap_or_else(|| "route_table.json".to_string());

    println!("Route Table Export Tool");
    println!("=======================");
    println!("Sample cells: {}", sample_count);
    println!("Output file:  {}", output_path);
    println!();

    // Sample cells within Berlin bounds
    let cells = sample_cells(sample_count, 42);
    println!("Sampled {} unique cells within Berlin bounds.", cells.len());

    // Use H3GridRouteProvider (always available)
    let provider = H3GridRouteProvider;
    let total_pairs = cells.len() * cells.len();
    println!(
        "Computing routes for {} pairs ({} x {} cells)...",
        total_pairs,
        cells.len(),
        cells.len()
    );

    let mut routes_found = 0usize;
    let mut total_distance_km = 0.0f64;

    // Build a simple JSON-serializable table (distance + duration only)
    let mut entries: Vec<serde_json::Value> = Vec::new();

    for (i, &from) in cells.iter().enumerate() {
        if i % 10 == 0 {
            eprint!(
                "\r  Progress: {}/{} origin cells ({} routes found)...",
                i,
                cells.len(),
                routes_found
            );
        }
        for &to in &cells {
            if from == to {
                continue;
            }
            if let Some(route) = provider.route(from, to) {
                routes_found += 1;
                total_distance_km += route.distance_km;
                entries.push(serde_json::json!({
                    "from": u64::from(from),
                    "to": u64::from(to),
                    "distance_km": route.distance_km,
                    "duration_secs": route.duration_secs,
                    "num_cells": route.cells.len(),
                }));
            }
        }
    }
    eprintln!();

    println!();
    println!("Results:");
    println!("  Routes found: {}", routes_found);
    println!(
        "  Average distance: {:.2} km",
        if routes_found > 0 {
            total_distance_km / routes_found as f64
        } else {
            0.0
        }
    );

    // Write to JSON
    let table = serde_json::json!({
        "metadata": {
            "provider": "H3GridRouteProvider",
            "sample_cells": cells.len(),
            "total_pairs": total_pairs,
            "routes_found": routes_found,
            "bounds": {
                "lat_min": LAT_MIN, "lat_max": LAT_MAX,
                "lng_min": LNG_MIN, "lng_max": LNG_MAX,
            }
        },
        "routes": entries,
    });

    match std::fs::write(&output_path, serde_json::to_string_pretty(&table).unwrap()) {
        Ok(()) => println!("  Written to: {}", output_path),
        Err(e) => eprintln!("  ERROR writing file: {}", e),
    }

    println!();
    println!("Note: This used H3GridRouteProvider (Haversine distances).");
    println!("For real road-network routes, enable OSRM features:");
    println!(
        "  cargo run --example route_export -p sim_experiments --features sim_core/osrm,sim_core/precomputed"
    );
}
