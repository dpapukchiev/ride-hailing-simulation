//! Scenario setup: configure spawners for riders and drivers.
//!
//! Uses spawners with inter-arrival time distributions to control spawn rates,
//! enabling variable supply and demand patterns.

use bevy_ecs::prelude::{Resource, World};

use crate::clock::SimulationClock;
use crate::distributions::{ExponentialInterArrival, UniformInterArrival};
use crate::matching::{CostBasedMatching, MatchingAlgorithmResource, SimpleMatching};
use crate::spawner::{DriverSpawner, DriverSpawnerConfig, RiderSpawner, RiderSpawnerConfig};
use crate::speed::SpeedModel;
use crate::telemetry::{SimSnapshotConfig, SimSnapshots, SimTelemetry};

/// Default bounding box: San Francisco Bay Area (approx).
const DEFAULT_LAT_MIN: f64 = 37.6;
const DEFAULT_LAT_MAX: f64 = 37.85;
const DEFAULT_LNG_MIN: f64 = -122.55;
const DEFAULT_LNG_MAX: f64 = -122.35;

/// Default time window for rider requests: 1 hour (simulation ms).
const DEFAULT_REQUEST_WINDOW_MS: u64 = 60 * 60 * 1000;

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
    /// Time window in simulation ms: riders spawn over this window.
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

/// Pick a random destination within [min_cells, max_cells] H3 distance from pickup.
/// Uses rejection sampling for efficiency when max_cells is large (avoids generating huge grid disks).
/// This function is exported for use by spawner systems.
pub fn random_destination<R: rand::Rng>(
    rng: &mut R,
    pickup: h3o::CellIndex,
    geo: &crate::spatial::GeoIndex,
    min_cells: u32,
    max_cells: u32,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
) -> h3o::CellIndex {
    use h3o::{CellIndex, LatLng, Resolution};
    use rand::Rng;
    
    let max_cells = max_cells.max(min_cells);
    
    fn random_cell_in_bounds<R: Rng>(rng: &mut R, lat_min: f64, lat_max: f64, lng_min: f64, lng_max: f64) -> CellIndex {
        let lat = rng.gen_range(lat_min..=lat_max);
        let lng = rng.gen_range(lng_min..=lng_max);
        let coord = LatLng::new(lat, lng).expect("valid lat/lng");
        coord.to_cell(Resolution::Nine)
    }
    
    fn cell_in_bounds(cell: CellIndex, lat_min: f64, lat_max: f64, lng_min: f64, lng_max: f64) -> bool {
        let coord: LatLng = cell.into();
        let lat = coord.lat();
        let lng = coord.lng();
        lat >= lat_min && lat <= lat_max && lng >= lng_min && lng <= lng_max
    }
    
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

/// Create a simple matching algorithm (first match within radius).
pub fn create_simple_matching() -> MatchingAlgorithmResource {
    MatchingAlgorithmResource::new(Box::new(SimpleMatching::default()))
}

/// Create a cost-based matching algorithm with the given ETA weight.
pub fn create_cost_based_matching(eta_weight: f64) -> MatchingAlgorithmResource {
    MatchingAlgorithmResource::new(Box::new(CostBasedMatching::new(eta_weight)))
}

/// Populates `world` with clock, telemetry, and spawner configurations.
/// Entities are spawned dynamically by spawner systems reacting to SimulationStarted events.
/// Caller must have already created `world`; this inserts resources and configures spawners.
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

    let seed = params.seed.unwrap_or(0);
    let request_window_ms = params.request_window_ms;
    let lat_min = params.lat_min;
    let lat_max = params.lat_max;
    let lng_min = params.lng_min;
    let lng_max = params.lng_max;
    let min_trip = params.min_trip_cells;
    let max_trip = params.max_trip_cells;

    // Create rider spawner: exponential distribution with average rate to spawn num_riders over request_window_ms
    let avg_rate_per_sec = if request_window_ms > 0 {
        (params.num_riders as f64) / (request_window_ms as f64 / 1000.0)
    } else {
        0.0
    };
    let rider_spawner_config = RiderSpawnerConfig {
        inter_arrival_dist: Box::new(ExponentialInterArrival::new(avg_rate_per_sec, seed)),
        lat_min,
        lat_max,
        lng_min,
        lng_max,
        min_trip_cells: min_trip,
        max_trip_cells: max_trip,
        start_time_ms: Some(0),
        end_time_ms: Some(request_window_ms),
        max_count: Some(params.num_riders),
        seed,
    };
    world.insert_resource(RiderSpawner::new(rider_spawner_config));

    // Create driver spawner: spawn all drivers upfront (uniform distribution with very short interval)
    // For simplicity, we'll spawn them all at time 0, but using a spawner allows future flexibility
    // Use a different seed offset for drivers to ensure independent randomness
    let driver_seed = seed.wrapping_add(0xdead_beef);
    let driver_spawner_config = DriverSpawnerConfig {
        inter_arrival_dist: Box::new(UniformInterArrival::new(1.0)), // 1ms between spawns
        lat_min,
        lat_max,
        lng_min,
        lng_max,
        start_time_ms: Some(0),
        end_time_ms: Some(1000), // Spawn within first second
        max_count: Some(params.num_drivers),
        seed: driver_seed,
    };
    world.insert_resource(DriverSpawner::new(driver_spawner_config));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_scenario_configures_spawners() {
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

        let rider_spawner = world.resource::<RiderSpawner>();
        assert_eq!(rider_spawner.config.max_count, Some(10));
        assert_eq!(rider_spawner.spawned_count, 0);

        let driver_spawner = world.resource::<DriverSpawner>();
        assert_eq!(driver_spawner.config.max_count, Some(3));
        assert_eq!(driver_spawner.spawned_count, 0);
    }
}
