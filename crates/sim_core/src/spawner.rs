//! Entity spawners: dynamically spawn riders and drivers based on distributions.
//!
//! Spawners use inter-arrival time distributions to control spawn rates, enabling
//! variable supply and demand patterns. They react to SimulationStarted events
//! and schedule their own spawn events.

use bevy_ecs::prelude::Resource;
use h3o::{CellIndex, LatLng, Resolution};

use crate::distributions::InterArrivalDistribution;

/// Common spawner state shared between rider and driver spawners.
#[derive(Debug)]
struct SpawnerState {
    /// Next spawn time in simulation ms.
    next_spawn_time_ms: u64,
    /// Number of entities spawned so far.
    spawned_count: usize,
    /// Whether spawning has been initialized.
    initialized: bool,
}

impl SpawnerState {
    fn new(start_time_ms: Option<u64>) -> Self {
        Self {
            next_spawn_time_ms: start_time_ms.unwrap_or(0),
            spawned_count: 0,
            initialized: false,
        }
    }
}

/// Common spawner configuration fields shared between rider and driver spawners.
trait SpawnerConfig {
    fn inter_arrival_dist(&self) -> &dyn InterArrivalDistribution;
    fn start_time_ms(&self) -> Option<u64>;
    fn end_time_ms(&self) -> Option<u64>;
    fn max_count(&self) -> Option<usize>;
}

impl SpawnerConfig for RiderSpawnerConfig {
    fn inter_arrival_dist(&self) -> &dyn InterArrivalDistribution {
        self.inter_arrival_dist.as_ref()
    }
    fn start_time_ms(&self) -> Option<u64> {
        self.start_time_ms
    }
    fn end_time_ms(&self) -> Option<u64> {
        self.end_time_ms
    }
    fn max_count(&self) -> Option<usize> {
        self.max_count
    }
}

impl SpawnerConfig for DriverSpawnerConfig {
    fn inter_arrival_dist(&self) -> &dyn InterArrivalDistribution {
        self.inter_arrival_dist.as_ref()
    }
    fn start_time_ms(&self) -> Option<u64> {
        self.start_time_ms
    }
    fn end_time_ms(&self) -> Option<u64> {
        self.end_time_ms
    }
    fn max_count(&self) -> Option<usize> {
        self.max_count
    }
}

/// Common spawner logic shared between rider and driver spawners.
fn should_spawn_common(
    state: &SpawnerState,
    config: &dyn SpawnerConfig,
    current_time_ms: u64,
) -> bool {
    // Check if we've hit max count
    if let Some(max) = config.max_count() {
        if state.spawned_count >= max {
            return false;
        }
    }

    // Check if we're past end time
    if let Some(end_time) = config.end_time_ms() {
        if current_time_ms > end_time {
            return false;
        }
    }

    // Check if we're before start time
    if let Some(start_time) = config.start_time_ms() {
        if current_time_ms < start_time {
            return false;
        }
    }

    // Check if it's time to spawn
    current_time_ms >= state.next_spawn_time_ms
}

/// Common advance logic shared between rider and driver spawners.
fn advance_common(
    state: &mut SpawnerState,
    config: &dyn SpawnerConfig,
    current_time_ms: u64,
) -> f64 {
    let inter_arrival_ms = config
        .inter_arrival_dist()
        .sample_ms(state.spawned_count as u64, current_time_ms);
    state.next_spawn_time_ms = current_time_ms + inter_arrival_ms.max(0.0) as u64;
    state.spawned_count += 1;
    inter_arrival_ms
}

/// Configuration for a rider spawner.
#[derive(Debug)]
pub struct RiderSpawnerConfig {
    /// Inter-arrival time distribution.
    pub inter_arrival_dist: Box<dyn InterArrivalDistribution>,
    /// Geographic bounds for spawn positions.
    pub lat_min: f64,
    pub lat_max: f64,
    pub lng_min: f64,
    pub lng_max: f64,
    /// Trip length bounds (H3 cells).
    pub min_trip_cells: u32,
    pub max_trip_cells: u32,
    /// Optional: start spawning at this time (ms). If None, starts immediately.
    pub start_time_ms: Option<u64>,
    /// Optional: stop spawning after this time (ms). If None, spawns indefinitely.
    pub end_time_ms: Option<u64>,
    /// Optional: maximum number of riders to spawn. If None, spawns indefinitely.
    pub max_count: Option<usize>,
    /// Number of riders to spawn immediately at simulation start (before scheduled spawning).
    pub initial_count: usize,
    /// Seed for RNG used for position/destination generation (for determinism).
    pub seed: u64,
}

/// Configuration for a driver spawner.
#[derive(Debug)]
pub struct DriverSpawnerConfig {
    /// Inter-arrival time distribution.
    pub inter_arrival_dist: Box<dyn InterArrivalDistribution>,
    /// Geographic bounds for spawn positions.
    pub lat_min: f64,
    pub lat_max: f64,
    pub lng_min: f64,
    pub lng_max: f64,
    /// Optional: start spawning at this time (ms). If None, starts immediately.
    pub start_time_ms: Option<u64>,
    /// Optional: stop spawning after this time (ms). If None, spawns indefinitely.
    pub end_time_ms: Option<u64>,
    /// Optional: maximum number of drivers to spawn. If None, spawns indefinitely.
    pub max_count: Option<usize>,
    /// Number of drivers to spawn immediately at simulation start (before scheduled spawning).
    pub initial_count: usize,
    /// Seed for RNG used for position generation (for determinism).
    pub seed: u64,
}

/// Active rider spawner resource that tracks spawning state.
#[derive(Debug, Resource)]
pub struct RiderSpawner {
    pub config: RiderSpawnerConfig,
    state: SpawnerState,
}

impl RiderSpawner {
    /// Create a new rider spawner from configuration.
    pub fn new(config: RiderSpawnerConfig) -> Self {
        Self {
            state: SpawnerState::new(config.start_time_ms),
            config,
        }
    }

    /// Check if the spawner should continue spawning at the current time.
    pub fn should_spawn(&self, current_time_ms: u64) -> bool {
        should_spawn_common(&self.state, &self.config, current_time_ms)
    }

    /// Advance the spawner to the next spawn time.
    /// Returns the inter-arrival time that was sampled (in ms).
    pub fn advance(&mut self, current_time_ms: u64) -> f64 {
        advance_common(&mut self.state, &self.config, current_time_ms)
    }

    /// Get the next spawn time in simulation ms.
    pub fn next_spawn_time_ms(&self) -> u64 {
        self.state.next_spawn_time_ms
    }

    /// Get the number of riders spawned so far.
    pub fn spawned_count(&self) -> usize {
        self.state.spawned_count
    }

    /// Get whether spawning has been initialized.
    pub fn initialized(&self) -> bool {
        self.state.initialized
    }

    /// Set whether spawning has been initialized.
    pub fn set_initialized(&mut self, initialized: bool) {
        self.state.initialized = initialized;
    }

    /// Set the next spawn time.
    pub fn set_next_spawn_time_ms(&mut self, time_ms: u64) {
        self.state.next_spawn_time_ms = time_ms;
    }

    /// Increment the spawned count (used for initial spawns).
    pub fn increment_spawned_count(&mut self) {
        self.state.spawned_count += 1;
    }
}

/// Active driver spawner resource that tracks spawning state.
#[derive(Debug, Resource)]
pub struct DriverSpawner {
    pub config: DriverSpawnerConfig,
    state: SpawnerState,
}

impl DriverSpawner {
    /// Create a new driver spawner from configuration.
    pub fn new(config: DriverSpawnerConfig) -> Self {
        Self {
            state: SpawnerState::new(config.start_time_ms),
            config,
        }
    }

    /// Check if the spawner should continue spawning at the current time.
    pub fn should_spawn(&self, current_time_ms: u64) -> bool {
        should_spawn_common(&self.state, &self.config, current_time_ms)
    }

    /// Advance the spawner to the next spawn time.
    /// Returns the inter-arrival time that was sampled (in ms).
    pub fn advance(&mut self, current_time_ms: u64) -> f64 {
        advance_common(&mut self.state, &self.config, current_time_ms)
    }

    /// Get the next spawn time in simulation ms.
    pub fn next_spawn_time_ms(&self) -> u64 {
        self.state.next_spawn_time_ms
    }

    /// Get the number of drivers spawned so far.
    pub fn spawned_count(&self) -> usize {
        self.state.spawned_count
    }

    /// Get whether spawning has been initialized.
    pub fn initialized(&self) -> bool {
        self.state.initialized
    }

    /// Set whether spawning has been initialized.
    pub fn set_initialized(&mut self, initialized: bool) {
        self.state.initialized = initialized;
    }

    /// Set the next spawn time.
    pub fn set_next_spawn_time_ms(&mut self, time_ms: u64) {
        self.state.next_spawn_time_ms = time_ms;
    }

    /// Increment the spawned count (used for initial spawns).
    pub fn increment_spawned_count(&mut self) {
        self.state.spawned_count += 1;
    }
}

/// Sample a random H3 cell (resolution 9) within the given lat/lng bounds.
///
/// # Errors
///
/// Returns an error if the generated coordinates are invalid (out of valid lat/lng range).
/// This should not happen with reasonable bounds, but can occur with corrupted data.
pub fn random_cell_in_bounds<R: rand::Rng>(
    rng: &mut R,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
) -> Result<CellIndex, String> {
    // Validate bounds before generating coordinates
    if lat_min < -90.0 || lat_max > 90.0 || lat_min > lat_max {
        return Err(format!(
            "Invalid latitude bounds: [{}, {}] (must be in [-90, 90] and min <= max)",
            lat_min, lat_max
        ));
    }
    if lng_min < -180.0 || lng_max > 180.0 || lng_min > lng_max {
        return Err(format!(
            "Invalid longitude bounds: [{}, {}] (must be in [-180, 180] and min <= max)",
            lng_min, lng_max
        ));
    }

    let lat = rng.gen_range(lat_min..=lat_max);
    let lng = rng.gen_range(lng_min..=lng_max);

    let coord = LatLng::new(lat, lng)
        .map_err(|e| format!("Invalid coordinates ({}, {}): {}", lat, lng, e))?;
    Ok(coord.to_cell(Resolution::Nine))
}
