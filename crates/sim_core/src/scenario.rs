//! Scenario setup: configure spawners for riders and drivers.
//!
//! Uses spawners with inter-arrival time distributions to control spawn rates,
//! enabling variable supply and demand patterns.

use bevy_ecs::prelude::{Resource, World};

use crate::clock::SimulationClock;
use crate::distributions::TimeOfDayDistribution;
use crate::matching::{
    CostBasedMatching, HungarianMatching, MatchingAlgorithmResource, SimpleMatching,
};
use crate::patterns::{apply_driver_patterns, apply_rider_patterns};
use crate::pricing::PricingConfig;
use crate::spatial::SpatialIndex;
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

/// Average multiplier for rider demand patterns.
/// Used to adjust base spawn rate to account for time-of-day variations.
/// The actual multipliers vary by hour (rush hours ~2.5-3.2x, night ~0.3-0.4x),
/// but the average across all hours is approximately 1.3.
const RIDER_DEMAND_AVERAGE_MULTIPLIER: f64 = 1.3;

/// Average multiplier for driver supply patterns.
/// Used to adjust base spawn rate to account for time-of-day variations.
/// Driver supply is more consistent than demand, with an average multiplier of approximately 1.2.
const DRIVER_SUPPLY_AVERAGE_MULTIPLIER: f64 = 1.2;

// Note: DEFAULT_ETA_WEIGHT is defined in matching/cost_based.rs to keep it with the algorithm

/// Type of matching algorithm to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchingAlgorithmType {
    Simple,
    CostBased,
    Hungarian,
}

/// Max H3 grid distance (cells) for matching rider to driver. 0 = same cell only.
#[derive(Debug, Clone, Copy, Default, Resource)]
pub struct MatchRadius(pub u32);

/// Simulation end time in milliseconds. When set, the runner stops processing events
/// once the next event would be at or after this timestamp (so the simulation "ends" at this time).
#[derive(Debug, Clone, Copy, Resource)]
pub struct SimulationEndTimeMs(pub u64);

/// Batch matching: run a global matching pass every N seconds instead of per-rider TryMatch.
#[derive(Debug, Clone, Copy, Resource)]
pub struct BatchMatchingConfig {
    /// When true, BatchMatchRun events are scheduled and per-rider TryMatch is not used.
    pub enabled: bool,
    /// Interval in seconds between batch matching runs.
    pub interval_secs: u64,
}

impl Default for BatchMatchingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_secs: 5,
        }
    }
}

/// Rider cancel window while waiting for pickup (seconds).
/// Uses a uniform distribution between min_wait_secs and max_wait_secs.
#[derive(Debug, Clone, Copy, Resource)]
pub struct RiderCancelConfig {
    pub min_wait_secs: u64,
    pub max_wait_secs: u64,
    /// Seed for RNG (for reproducibility).
    pub seed: u64,
}

impl Default for RiderCancelConfig {
    fn default() -> Self {
        Self {
            min_wait_secs: 120,
            max_wait_secs: 2400,
            seed: 0,
        }
    }
}

/// Rider quote behavior: reject/retry and give-up after max rejections.
#[derive(Debug, Clone, Copy, Resource)]
pub struct RiderQuoteConfig {
    /// Maximum number of quote rejections before rider gives up.
    pub max_quote_rejections: u32,
    /// Delay in seconds before requesting another quote after rejection.
    pub re_quote_delay_secs: u64,
    /// Probability (0.0–1.0) that rider accepts the quote when within price/ETA limits.
    pub accept_probability: f64,
    /// Seed for RNG (for reproducibility).
    pub seed: u64,
    /// Maximum willingness to pay; reject quote if fare exceeds this.
    pub max_willingness_to_pay: f64,
    /// Maximum acceptable ETA to pickup (ms); reject quote if eta_ms exceeds this.
    pub max_acceptable_eta_ms: u64,
}

impl Default for RiderQuoteConfig {
    fn default() -> Self {
        Self {
            max_quote_rejections: 3,
            re_quote_delay_secs: 10,
            accept_probability: 0.8,
            seed: 0,
            max_willingness_to_pay: 100.0,
            max_acceptable_eta_ms: 600_000, // 10 min
        }
    }
}

/// Driver decision behavior: stochastic logit model for accept/reject decisions.
#[derive(Debug, Clone, Copy, Resource)]
pub struct DriverDecisionConfig {
    /// Seed for RNG (for reproducibility).
    pub seed: u64,
    /// Weight for fare attractiveness (higher fare increases acceptance).
    pub fare_weight: f64,
    /// Penalty per km of pickup distance (longer pickup distance decreases acceptance).
    pub pickup_distance_penalty: f64,
    /// Bonus per km of trip distance (longer trips increase acceptance).
    pub trip_distance_bonus: f64,
    /// Weight for earnings progress (drivers closer to target are less likely to accept).
    pub earnings_progress_weight: f64,
    /// Penalty for fatigue (more fatigued drivers are less likely to accept).
    pub fatigue_penalty: f64,
    /// Base acceptance score before factors are applied.
    pub base_acceptance_score: f64,
}

impl Default for DriverDecisionConfig {
    fn default() -> Self {
        Self {
            seed: 0,
            fare_weight: 0.1,
            pickup_distance_penalty: -2.0,
            trip_distance_bonus: 0.5,
            earnings_progress_weight: -0.5,
            fatigue_penalty: -1.0,
            base_acceptance_score: 1.0,
        }
    }
}

/// Parameters for building a scenario.
#[derive(Debug, Clone)]
pub struct ScenarioParams {
    pub num_riders: usize,
    pub num_drivers: usize,
    /// Number of riders to spawn immediately at simulation start (before scheduled spawning).
    pub initial_rider_count: usize,
    /// Number of drivers to spawn immediately at simulation start (before scheduled spawning).
    pub initial_driver_count: usize,
    /// Random seed for reproducibility (optional; if None, uses thread rng).
    pub seed: Option<u64>,
    /// Bounding box for random positions (lat/lng degrees).
    pub lat_min: f64,
    pub lat_max: f64,
    pub lng_min: f64,
    pub lng_max: f64,
    /// Time window in simulation ms: riders spawn over this window.
    pub request_window_ms: u64,
    /// Time window in simulation ms: drivers spawn over this window.
    pub driver_spread_ms: u64,
    /// Max H3 grid distance for matching (0 = same cell only).
    pub match_radius: u32,
    /// Min/max trip length in H3 cells (travel time depends on movement speed).
    pub min_trip_cells: u32,
    pub max_trip_cells: u32,
    /// Epoch in milliseconds (real-world time corresponding to simulation time 0).
    /// Used for time-of-day distributions. If None, defaults to 0.
    pub epoch_ms: Option<i64>,
    /// Pricing configuration. If None, uses default PricingConfig.
    pub pricing_config: Option<PricingConfig>,
    /// Rider quote configuration. If None, uses default RiderQuoteConfig.
    pub rider_quote_config: Option<RiderQuoteConfig>,
    /// Driver decision configuration. If None, uses default DriverDecisionConfig.
    pub driver_decision_config: Option<DriverDecisionConfig>,
    /// Simulation end time in ms. When set, the runner stops when the next event is at or after this time.
    pub simulation_end_time_ms: Option<u64>,
    /// Matching algorithm type. If None, defaults to Hungarian.
    pub matching_algorithm_type: Option<MatchingAlgorithmType>,
    /// Whether batch matching is enabled. If None, defaults to true.
    pub batch_matching_enabled: Option<bool>,
    /// Batch matching interval in seconds. If None, defaults to 5.
    pub batch_interval_secs: Option<u64>,
    /// ETA weight for cost-based and Hungarian matching algorithms. If None, uses DEFAULT_ETA_WEIGHT.
    pub eta_weight: Option<f64>,
}

impl Default for ScenarioParams {
    fn default() -> Self {
        Self {
            num_riders: 500,
            num_drivers: 100,
            initial_rider_count: 0,
            initial_driver_count: 0,
            seed: None,
            lat_min: DEFAULT_LAT_MIN,
            lat_max: DEFAULT_LAT_MAX,
            lng_min: DEFAULT_LNG_MIN,
            lng_max: DEFAULT_LNG_MAX,
            request_window_ms: DEFAULT_REQUEST_WINDOW_MS,
            driver_spread_ms: DEFAULT_REQUEST_WINDOW_MS,
            match_radius: 0,
            min_trip_cells: 5,
            max_trip_cells: 60,
            epoch_ms: None,
            pricing_config: None,
            rider_quote_config: None,
            driver_decision_config: None,
            simulation_end_time_ms: None,
            matching_algorithm_type: None,
            batch_matching_enabled: None,
            batch_interval_secs: None,
            eta_weight: None,
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

    /// Set the driver spread time window in hours (drivers spawn over [0, hours] sim time).
    pub fn with_driver_spread_hours(mut self, hours: u64) -> Self {
        self.driver_spread_ms = hours * 60 * 60 * 1000;
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

    /// Set the epoch in milliseconds (real-world time corresponding to simulation time 0).
    pub fn with_epoch_ms(mut self, epoch_ms: i64) -> Self {
        self.epoch_ms = Some(epoch_ms);
        self
    }

    /// Set pricing configuration.
    pub fn with_pricing_config(mut self, pricing_config: PricingConfig) -> Self {
        self.pricing_config = Some(pricing_config);
        self
    }

    /// Set simulation end time in ms. Runner stops when the next event is at or after this time.
    pub fn with_simulation_end_time_ms(mut self, end_ms: u64) -> Self {
        self.simulation_end_time_ms = Some(end_ms);
        self
    }

    /// Set rider quote configuration.
    pub fn with_rider_quote_config(mut self, rider_quote_config: RiderQuoteConfig) -> Self {
        self.rider_quote_config = Some(rider_quote_config);
        self
    }

    /// Set driver decision configuration.
    pub fn with_driver_decision_config(
        mut self,
        driver_decision_config: DriverDecisionConfig,
    ) -> Self {
        self.driver_decision_config = Some(driver_decision_config);
        self
    }
}

/// Threshold for choosing between grid_disk and rejection sampling strategies.
/// For distances <= this threshold, grid_disk is more efficient.
/// For larger distances, rejection sampling avoids generating huge grid disks.
const GRID_DISK_THRESHOLD: u32 = 20;

/// Maximum attempts for rejection sampling before falling back to grid_disk.
const MAX_REJECTION_SAMPLING_ATTEMPTS: usize = 2000;

/// Helper function to check if a cell is within geographic bounds.
fn cell_in_bounds(
    cell: h3o::CellIndex,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
) -> bool {
    let coord: h3o::LatLng = cell.into();
    let lat = coord.lat();
    let lng = coord.lng();
    lat >= lat_min && lat <= lat_max && lng >= lng_min && lng <= lng_max
}

/// Helper function to check if a cell is within the desired distance range from pickup.
fn is_valid_destination(
    cell: h3o::CellIndex,
    pickup: h3o::CellIndex,
    min_cells: u32,
    max_cells: u32,
) -> bool {
    pickup
        .grid_distance(cell)
        .map(|d| d >= min_cells as i32 && d <= max_cells as i32)
        .unwrap_or(false)
}

/// Strategy for small distances: generate grid disk and filter candidates.
/// More efficient for small radii where the disk size is manageable.
#[allow(clippy::too_many_arguments)]
fn grid_disk_strategy<R: rand::Rng>(
    rng: &mut R,
    pickup: h3o::CellIndex,
    geo: &crate::spatial::GeoIndex,
    min_cells: u32,
    max_cells: u32,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
) -> Option<h3o::CellIndex> {
    let disk = geo.grid_disk(pickup, max_cells);
    let candidates: Vec<h3o::CellIndex> = disk
        .into_iter()
        .filter(|c| {
            is_valid_destination(*c, pickup, min_cells, max_cells)
                && cell_in_bounds(*c, lat_min, lat_max, lng_min, lng_max)
        })
        .collect();

    if candidates.is_empty() {
        None
    } else {
        let i = rng.gen_range(0..candidates.len());
        Some(candidates[i])
    }
}

/// Strategy for large distances: rejection sampling.
/// Randomly samples cells within bounds and checks if they match the distance requirement.
/// This avoids generating huge grid disks (e.g., 33k+ cells for k=105).
fn rejection_sampling_strategy<R: rand::Rng>(
    rng: &mut R,
    pickup: h3o::CellIndex,
    min_cells: u32,
    max_cells: u32,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
) -> Option<h3o::CellIndex> {
    use crate::spawner::random_cell_in_bounds;

    for _ in 0..MAX_REJECTION_SAMPLING_ATTEMPTS {
        let cell = match random_cell_in_bounds(rng, lat_min, lat_max, lng_min, lng_max) {
            Ok(cell) => cell,
            Err(_) => {
                // If coordinate generation fails, fallback to center of bounds
                let lat = (lat_min + lat_max) / 2.0;
                let lng = (lng_min + lng_max) / 2.0;
                h3o::LatLng::new(lat, lng)
                    .ok()
                    .map(|c| c.to_cell(h3o::Resolution::Nine))
                    .unwrap_or(pickup)
            }
        };

        if is_valid_destination(cell, pickup, min_cells, max_cells) {
            return Some(cell);
        }
    }

    None
}

/// Fallback strategy: use a smaller grid_disk with radius between min and max.
/// Used when rejection sampling fails to find a valid destination.
#[allow(clippy::too_many_arguments)]
fn fallback_grid_disk_strategy<R: rand::Rng>(
    rng: &mut R,
    pickup: h3o::CellIndex,
    geo: &crate::spatial::GeoIndex,
    min_cells: u32,
    max_cells: u32,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
) -> Option<h3o::CellIndex> {
    // Use a radius closer to min_cells to reduce the number of cells generated
    let fallback_radius = (min_cells + max_cells) / 2;
    let disk = geo.grid_disk(pickup, fallback_radius);
    let candidates: Vec<h3o::CellIndex> = disk
        .into_iter()
        .filter(|c| {
            is_valid_destination(*c, pickup, min_cells, max_cells)
                && cell_in_bounds(*c, lat_min, lat_max, lng_min, lng_max)
        })
        .collect();

    if candidates.is_empty() {
        None
    } else {
        let i = rng.gen_range(0..candidates.len());
        Some(candidates[i])
    }
}

/// Pick a random destination within [min_cells, max_cells] H3 distance from pickup.
/// Uses rejection sampling for efficiency when max_cells is large (avoids generating huge grid disks).
/// This function is exported for use by spawner systems.
#[allow(clippy::too_many_arguments)]
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
    let max_cells = max_cells.max(min_cells);

    // For small radii, use grid_disk approach (more efficient)
    if max_cells <= GRID_DISK_THRESHOLD {
        if let Some(destination) = grid_disk_strategy(
            rng, pickup, geo, min_cells, max_cells, lat_min, lat_max, lng_min, lng_max,
        ) {
            return destination;
        }
        // If grid_disk fails, fall through to rejection sampling
    }

    // For large radii, use rejection sampling (much faster)
    if let Some(destination) = rejection_sampling_strategy(
        rng, pickup, min_cells, max_cells, lat_min, lat_max, lng_min, lng_max,
    ) {
        return destination;
    }

    // Fallback: try a smaller grid_disk
    if let Some(destination) = fallback_grid_disk_strategy(
        rng, pickup, geo, min_cells, max_cells, lat_min, lat_max, lng_min, lng_max,
    ) {
        return destination;
    }

    // Last resort: return pickup
    pickup
}

/// Create a simple matching algorithm (first match within radius).
pub fn create_simple_matching() -> MatchingAlgorithmResource {
    MatchingAlgorithmResource::new(Box::new(SimpleMatching))
}

/// Create a cost-based matching algorithm with the given ETA weight.
pub fn create_cost_based_matching(eta_weight: f64) -> MatchingAlgorithmResource {
    MatchingAlgorithmResource::new(Box::new(CostBasedMatching::new(eta_weight)))
}

/// Create a Hungarian (global batch) matching algorithm with the given ETA weight.
pub fn create_hungarian_matching(eta_weight: f64) -> MatchingAlgorithmResource {
    MatchingAlgorithmResource::new(Box::new(HungarianMatching::new(eta_weight)))
}

/// Create a realistic time-of-day pattern for rider demand.
/// Uses patterns from the patterns module.
fn create_rider_time_of_day_pattern(
    base_rate_per_sec: f64,
    epoch_ms: i64,
    seed: u64,
) -> TimeOfDayDistribution {
    let dist = TimeOfDayDistribution::new(base_rate_per_sec, epoch_ms, seed);
    apply_rider_patterns(dist)
}

/// Create a realistic time-of-day pattern for driver supply.
/// Uses patterns from the patterns module.
fn create_driver_time_of_day_pattern(
    base_rate_per_sec: f64,
    epoch_ms: i64,
    seed: u64,
) -> TimeOfDayDistribution {
    let dist = TimeOfDayDistribution::new(base_rate_per_sec, epoch_ms, seed);
    apply_driver_patterns(dist)
}

/// Populates `world` with clock, telemetry, and spawner configurations.
/// Entities are spawned dynamically by spawner systems reacting to SimulationStarted events.
/// Caller must have already created `world`; this inserts resources and configures spawners.
pub fn build_scenario(world: &mut World, params: ScenarioParams) {
    let epoch_ms = params.epoch_ms.unwrap_or(0);
    let mut clock = SimulationClock::default();
    clock.set_epoch_ms(epoch_ms);
    world.insert_resource(clock);

    world.insert_resource(SimTelemetry::default());
    world.insert_resource(SimSnapshotConfig::default());
    world.insert_resource(SimSnapshots::default());

    // Only enable spatial index for larger scenarios where it provides benefit
    // Threshold: enable if total entities (riders + drivers) > 200
    // For smaller scenarios, full scans are fast enough and spatial index overhead hurts
    let total_entities = params.num_riders + params.num_drivers;
    if total_entities > 200 {
        world.insert_resource(SpatialIndex::new());
    }

    world.insert_resource(MatchRadius(params.match_radius));
    world.insert_resource(BatchMatchingConfig {
        enabled: params.batch_matching_enabled.unwrap_or(true),
        interval_secs: params.batch_interval_secs.unwrap_or(5),
    });
    if let Some(end_ms) = params.simulation_end_time_ms {
        world.insert_resource(SimulationEndTimeMs(end_ms));
    }
    let seed = params.seed.unwrap_or(0);
    world.insert_resource(RiderCancelConfig {
        min_wait_secs: 120,
        max_wait_secs: 2400,
        seed: seed.wrapping_add(0xcafe_babe), // Use a different seed offset for cancellation
    });
    world.insert_resource(
        params
            .rider_quote_config
            .unwrap_or_else(|| RiderQuoteConfig {
                max_quote_rejections: 3,
                re_quote_delay_secs: 10,
                accept_probability: 0.8,
                seed: seed.wrapping_add(0x0071_1073_beef),
                max_willingness_to_pay: 100.0,
                max_acceptable_eta_ms: 600_000,
            }),
    );
    world.insert_resource(
        params
            .driver_decision_config
            .unwrap_or_else(|| DriverDecisionConfig {
                seed: seed.wrapping_add(0xdead_beef),
                ..Default::default()
            }),
    );
    world.insert_resource(SpeedModel::new(params.seed.map(|seed| seed ^ 0x5eed_cafe)));
    // Set matching algorithm based on params
    let eta_weight = params
        .eta_weight
        .unwrap_or(crate::matching::DEFAULT_ETA_WEIGHT);
    let algorithm = match params
        .matching_algorithm_type
        .unwrap_or(MatchingAlgorithmType::Hungarian)
    {
        MatchingAlgorithmType::Simple => create_simple_matching(),
        MatchingAlgorithmType::CostBased => create_cost_based_matching(eta_weight),
        MatchingAlgorithmType::Hungarian => create_hungarian_matching(eta_weight),
    };
    world.insert_resource(algorithm);
    // Insert pricing config (use provided or default)
    world.insert_resource(params.pricing_config.unwrap_or_default());

    let seed = params.seed.unwrap_or(0);
    let request_window_ms = params.request_window_ms;
    let driver_spread_ms = params.driver_spread_ms;
    let lat_min = params.lat_min;
    let lat_max = params.lat_max;
    let lng_min = params.lng_min;
    let lng_max = params.lng_max;
    let min_trip = params.min_trip_cells;
    let max_trip = params.max_trip_cells;

    // Create rider spawner: time-of-day distribution with average rate to spawn num_riders over request_window_ms
    // The base rate is calculated to achieve the target number of riders, but actual spawn rate varies by time of day
    // Note: initial_rider_count are spawned immediately, so we subtract them from the scheduled count
    let scheduled_rider_count = params.num_riders.saturating_sub(params.initial_rider_count);
    let avg_rate_per_sec = if request_window_ms > 0 && scheduled_rider_count > 0 {
        (scheduled_rider_count as f64) / (request_window_ms as f64 / 1000.0)
    } else {
        0.0
    };
    // Adjust base rate to account for multipliers (average multiplier is ~1.3)
    let base_rate_per_sec = avg_rate_per_sec / RIDER_DEMAND_AVERAGE_MULTIPLIER;

    let rider_spawner_config = RiderSpawnerConfig {
        inter_arrival_dist: Box::new(create_rider_time_of_day_pattern(
            base_rate_per_sec,
            epoch_ms,
            seed,
        )),
        lat_min,
        lat_max,
        lng_min,
        lng_max,
        min_trip_cells: min_trip,
        max_trip_cells: max_trip,
        start_time_ms: Some(0),
        end_time_ms: Some(request_window_ms),
        max_count: Some(scheduled_rider_count),
        initial_count: params.initial_rider_count,
        seed,
    };
    world.insert_resource(RiderSpawner::new(rider_spawner_config));

    // Create driver spawner: time-of-day distribution for driver supply
    // Drivers spawn continuously over the driver_spread_ms window with time-varying rates
    // Note: initial_driver_count are spawned immediately, so we subtract them from the scheduled count
    let driver_seed = seed.wrapping_add(0xdead_beef);
    let scheduled_driver_count = params
        .num_drivers
        .saturating_sub(params.initial_driver_count);
    let driver_base_rate_per_sec = if driver_spread_ms > 0 && scheduled_driver_count > 0 {
        (scheduled_driver_count as f64)
            / (driver_spread_ms as f64 / 1000.0)
            / DRIVER_SUPPLY_AVERAGE_MULTIPLIER
    } else {
        0.0
    };

    let driver_spawner_config = DriverSpawnerConfig {
        inter_arrival_dist: Box::new(create_driver_time_of_day_pattern(
            driver_base_rate_per_sec,
            epoch_ms,
            driver_seed,
        )),
        lat_min,
        lat_max,
        lng_min,
        lng_max,
        start_time_ms: Some(0),
        end_time_ms: Some(driver_spread_ms),
        max_count: Some(scheduled_driver_count),
        initial_count: params.initial_driver_count,
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
        assert_eq!(rider_spawner.spawned_count(), 0);

        let driver_spawner = world.resource::<DriverSpawner>();
        assert_eq!(driver_spawner.config.max_count, Some(3));
        assert_eq!(driver_spawner.spawned_count(), 0);
    }

    #[test]
    fn build_scenario_handles_empty_scenarios() {
        let mut world = World::new();
        build_scenario(
            &mut world,
            ScenarioParams {
                num_riders: 0,
                num_drivers: 0,
                seed: Some(42),
                ..Default::default()
            },
        );

        let rider_spawner = world.resource::<RiderSpawner>();
        assert_eq!(rider_spawner.config.max_count, Some(0));

        let driver_spawner = world.resource::<DriverSpawner>();
        assert_eq!(driver_spawner.config.max_count, Some(0));
    }

    #[test]
    fn build_scenario_handles_zero_match_radius() {
        let mut world = World::new();
        build_scenario(
            &mut world,
            ScenarioParams {
                num_riders: 5,
                num_drivers: 2,
                match_radius: 0, // Same cell only
                seed: Some(42),
                ..Default::default()
            },
        );

        let match_radius = world.resource::<MatchRadius>();
        assert_eq!(match_radius.0, 0);
    }

    #[test]
    fn build_scenario_handles_initial_counts() {
        let mut world = World::new();
        build_scenario(
            &mut world,
            ScenarioParams {
                num_riders: 10,
                num_drivers: 5,
                initial_rider_count: 3,
                initial_driver_count: 2,
                seed: Some(42),
                ..Default::default()
            },
        );

        let rider_spawner = world.resource::<RiderSpawner>();
        // max_count should be total - initial (scheduled spawns only)
        assert_eq!(rider_spawner.config.max_count, Some(7));
        assert_eq!(rider_spawner.config.initial_count, 3);

        let driver_spawner = world.resource::<DriverSpawner>();
        assert_eq!(driver_spawner.config.max_count, Some(3));
        assert_eq!(driver_spawner.config.initial_count, 2);
    }
}
