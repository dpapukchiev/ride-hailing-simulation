//! Scenario setup: spawn riders and drivers with random positions and request times.
//!
//! Uses a geographic bounding box to sample random H3 cells (resolution 9) and
//! spreads rider request events over a configurable time window.

use bevy_ecs::prelude::{Resource, World};
use h3o::{CellIndex, LatLng, Resolution};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::VecDeque;

use crate::clock::{EventKind, SimulationClock};
use crate::ecs::{Driver, DriverState, Position};
use crate::matching::{CostBasedMatching, MatchingAlgorithmResource, SimpleMatching};
use crate::spatial::GeoIndex;
use crate::speed::SpeedModel;
use crate::telemetry::{SimSnapshotConfig, SimSnapshots, SimTelemetry};

/// Default bounding box: San Francisco Bay Area (approx).
const DEFAULT_LAT_MIN: f64 = 37.6;
const DEFAULT_LAT_MAX: f64 = 37.85;
const DEFAULT_LNG_MIN: f64 = -122.55;
const DEFAULT_LNG_MAX: f64 = -122.35;

/// Default time window for rider requests: 1 hour (simulation ms).
const DEFAULT_REQUEST_WINDOW_MS: u64 = 60 * 60 * 1000;

/// Pending rider spawn data (for just-in-time spawning when RequestInbound fires).
#[derive(Debug, Clone)]
pub struct PendingRider {
    pub position: CellIndex,
    pub destination: CellIndex,
    pub request_time_ms: u64,
}

/// Queue of riders to spawn at their request time (FIFO).
#[derive(Debug, Clone, Default, Resource)]
pub struct PendingRiders(pub VecDeque<PendingRider>);

/// Max H3 grid distance (cells) for matching rider to driver. 0 = same cell only.
#[derive(Debug, Clone, Copy, Default, Resource)]
pub struct MatchRadius(pub u32);

/// Rider cancel window while waiting for pickup (seconds).
#[derive(Debug, Clone, Copy, Resource)]
pub struct RiderCancelConfig {
    pub min_wait_secs: u64,
    pub max_wait_secs: u64,
}

impl Default for RiderCancelConfig {
    fn default() -> Self {
        Self {
            min_wait_secs: 120,
            max_wait_secs: 2400,
        }
    }
}

/// Parameters for building a scenario.
#[derive(Debug, Clone)]
pub struct ScenarioParams {
    pub num_riders: usize,
    pub num_drivers: usize,
    /// Random seed for reproducibility (optional; if None, uses thread rng).
    pub seed: Option<u64>,
    /// Bounding box for random positions (lat/lng degrees).
    pub lat_min: f64,
    pub lat_max: f64,
    pub lng_min: f64,
    pub lng_max: f64,
    /// Time window in simulation ms: rider RequestInbound times are uniform in [0, request_window_ms].
    pub request_window_ms: u64,
    /// Max H3 grid distance for matching (0 = same cell only).
    pub match_radius: u32,
    /// Min/max trip length in H3 cells (travel time depends on movement speed).
    pub min_trip_cells: u32,
    pub max_trip_cells: u32,
}

impl Default for ScenarioParams {
    fn default() -> Self {
        Self {
            num_riders: 500,
            num_drivers: 100,
            seed: None,
            lat_min: DEFAULT_LAT_MIN,
            lat_max: DEFAULT_LAT_MAX,
            lng_min: DEFAULT_LNG_MIN,
            lng_max: DEFAULT_LNG_MAX,
            request_window_ms: DEFAULT_REQUEST_WINDOW_MS,
            match_radius: 0,
            min_trip_cells: 5,
            max_trip_cells: 60,
        }
    }
}

impl ScenarioParams {
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Set the request time window in hours (riders request uniformly in [0, hours] sim time).
    pub fn with_request_window_hours(mut self, hours: u64) -> Self {
        self.request_window_ms = hours * 60 * 60 * 1000;
        self
    }

    /// Match riders to drivers within this H3 grid distance (0 = same cell only).
    pub fn with_match_radius(mut self, radius: u32) -> Self {
        self.match_radius = radius;
        self
    }

    /// Trip length in H3 cells: min..=max (travel time depends on movement speed).
    pub fn with_trip_duration_cells(mut self, min_cells: u32, max_cells: u32) -> Self {
        self.min_trip_cells = min_cells;
        self.max_trip_cells = max_cells;
        self
    }
}

/// Sample a random H3 cell (resolution 9) within the given lat/lng bounds.
fn random_cell_in_bounds<R: Rng>(rng: &mut R, lat_min: f64, lat_max: f64, lng_min: f64, lng_max: f64) -> CellIndex {
    let lat = rng.gen_range(lat_min..=lat_max);
    let lng = rng.gen_range(lng_min..=lng_max);
    let coord = LatLng::new(lat, lng).expect("valid lat/lng");
    coord.to_cell(Resolution::Nine)
}

/// Pick a random destination within [min_cells, max_cells] H3 distance from pickup.
/// Uses rejection sampling for efficiency when max_cells is large (avoids generating huge grid disks).
fn random_destination<R: Rng>(
    rng: &mut R,
    pickup: CellIndex,
    geo: &GeoIndex,
    min_cells: u32,
    max_cells: u32,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
) -> CellIndex {
    let max_cells = max_cells.max(min_cells);
    
    // For small radii, use the original grid_disk approach (more efficient)
    // For large radii, use rejection sampling (much faster)
    const GRID_DISK_THRESHOLD: u32 = 20;
    
    if max_cells <= GRID_DISK_THRESHOLD {
        // Original approach: generate disk and filter
        let disk = geo.grid_disk(pickup, max_cells);
        let candidates: Vec<CellIndex> = disk
            .into_iter()
            .filter(|c| {
                pickup
                    .grid_distance(*c)
                    .map(|d| d >= min_cells as i32 && d <= max_cells as i32)
                    .unwrap_or(false)
                    && cell_in_bounds(*c, lat_min, lat_max, lng_min, lng_max)
            })
            .collect();
        if candidates.is_empty() {
            return pickup;
        }
        let i = rng.gen_range(0..candidates.len());
        return candidates[i];
    }
    
    // Rejection sampling: randomly sample cells within bounds and check distance
    // This avoids generating huge grid disks (e.g., 33k+ cells for k=105)
    // For a 25km city with max_trip_km=25, we need ~105 cells distance
    // The probability of hitting the right distance range is reasonable with enough attempts
    const MAX_ATTEMPTS: usize = 2000;
    for _ in 0..MAX_ATTEMPTS {
        let cell = random_cell_in_bounds(rng, lat_min, lat_max, lng_min, lng_max);
        if let Ok(distance) = pickup.grid_distance(cell) {
            let distance_u32 = distance as u32;
            if distance_u32 >= min_cells && distance_u32 <= max_cells {
                return cell;
            }
        }
    }
    
    // Fallback: if rejection sampling fails (unlikely), try a smaller grid_disk
    // Use a radius closer to min_cells to reduce the number of cells generated
    let fallback_radius = (min_cells + max_cells) / 2;
    let disk = geo.grid_disk(pickup, fallback_radius);
    let candidates: Vec<CellIndex> = disk
        .into_iter()
        .filter(|c| {
            pickup
                .grid_distance(*c)
                .map(|d| d >= min_cells as i32 && d <= max_cells as i32)
                .unwrap_or(false)
                && cell_in_bounds(*c, lat_min, lat_max, lng_min, lng_max)
        })
        .collect();
    if !candidates.is_empty() {
        let i = rng.gen_range(0..candidates.len());
        return candidates[i];
    }
    
    // Last resort: return pickup
    pickup
}

fn cell_in_bounds(cell: CellIndex, lat_min: f64, lat_max: f64, lng_min: f64, lng_max: f64) -> bool {
    let coord: LatLng = cell.into();
    let lat = coord.lat();
    let lng = coord.lng();
    lat >= lat_min && lat <= lat_max && lng >= lng_min && lng <= lng_max
}

/// Create a simple matching algorithm (first match within radius).
pub fn create_simple_matching() -> MatchingAlgorithmResource {
    MatchingAlgorithmResource::new(Box::new(SimpleMatching::default()))
}

/// Create a cost-based matching algorithm with the given ETA weight.
pub fn create_cost_based_matching(eta_weight: f64) -> MatchingAlgorithmResource {
    MatchingAlgorithmResource::new(Box::new(CostBasedMatching::new(eta_weight)))
}

/// Populates `world` with clock, telemetry, drivers, and scheduled RequestInbound events.
/// Riders are spawned just-in-time when their RequestInbound event fires.
/// Caller must have already created `world`; this inserts resources and spawns entities.
pub fn build_scenario(world: &mut World, params: ScenarioParams) {
    world.insert_resource(SimulationClock::default());
    world.insert_resource(SimTelemetry::default());
    world.insert_resource(SimSnapshotConfig::default());
    world.insert_resource(SimSnapshots::default());
    world.insert_resource(MatchRadius(params.match_radius));
    world.insert_resource(RiderCancelConfig::default());
    world.insert_resource(SpeedModel::new(params.seed.map(|seed| seed ^ 0x5eed_cafe)));
    // Default to cost-based matching with weight 0.1
    world.insert_resource(create_cost_based_matching(0.1));

    let mut rng: StdRng = match params.seed {
        Some(seed) => StdRng::seed_from_u64(seed),
        None => StdRng::from_entropy(),
    };

    let geo = GeoIndex::default();
    let lat_min = params.lat_min;
    let lat_max = params.lat_max;
    let lng_min = params.lng_min;
    let lng_max = params.lng_max;
    let request_window_ms = params.request_window_ms;
    let min_trip = params.min_trip_cells;
    let max_trip = params.max_trip_cells;

    // Generate pending rider data and schedule their request events
    let mut pending_riders = Vec::with_capacity(params.num_riders);
    for _ in 0..params.num_riders {
        let cell = random_cell_in_bounds(&mut rng, lat_min, lat_max, lng_min, lng_max);
        let destination = random_destination(
            &mut rng,
            cell,
            &geo,
            min_trip,
            max_trip,
            lat_min,
            lat_max,
            lng_min,
            lng_max,
        );
        let request_time_ms = rng.gen_range(0..=request_window_ms);

        pending_riders.push((
            request_time_ms,
            PendingRider {
                position: cell,
                destination,
                request_time_ms,
            },
        ));
    }

    // Sort by request time so we can pop from front in order
    pending_riders.sort_by_key(|(time, _)| *time);

    // Schedule RequestInbound events (without entity subjects - riders don't exist yet)
    let mut clock = world.resource_mut::<SimulationClock>();
    for (request_time_ms, _) in &pending_riders {
        clock.schedule_at(*request_time_ms, EventKind::RequestInbound, None);
    }
    drop(clock);

    // Store pending riders in resource
    world.insert_resource(PendingRiders(
        pending_riders.into_iter().map(|(_, rider)| rider).collect(),
    ));

    // Spawn all drivers upfront (they're always in the system)
    for _ in 0..params.num_drivers {
        let cell = random_cell_in_bounds(&mut rng, lat_min, lat_max, lng_min, lng_max);
        world.spawn((
            Driver {
                state: DriverState::Idle,
                matched_rider: None,
            },
            Position(cell),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_scenario_prepares_pending_riders_and_spawns_drivers() {
        let mut world = World::new();
        build_scenario(
            &mut world,
            ScenarioParams {
                num_riders: 10,
                num_drivers: 3,
                seed: Some(42),
                ..Default::default()
            },
        );

        let pending_riders = world.resource::<PendingRiders>();
        assert_eq!(pending_riders.0.len(), 10, "10 pending riders");

        let driver_count = world.query::<&Driver>().iter(&world).count();
        assert_eq!(driver_count, 3);

        let clock = world.resource::<SimulationClock>();
        assert_eq!(clock.pending_event_count(), 10, "one RequestInbound per rider");
    }
}
