//! Entity spawners: dynamically spawn riders and drivers based on distributions.
//!
//! Spawners use inter-arrival time distributions to control spawn rates, enabling
//! variable supply and demand patterns. They react to SimulationStarted events
//! and schedule their own spawn events.

use bevy_ecs::prelude::Resource;
use h3o::{CellIndex, LatLng, Resolution};
use serde::{Deserialize, Serialize};

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

// ---------------------------------------------------------------------------
// Spawn weighting (Phase 6: realistic spawn locations)
// ---------------------------------------------------------------------------

/// A weighted entry: an H3 cell with a relative weight.
/// Higher weights mean more spawns near this cell.
#[derive(Clone, Debug)]
pub struct WeightedCell {
    pub cell: CellIndex,
    pub weight: f64,
}

/// Configuration for spawn location weighting.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub enum SpawnWeightingKind {
    /// Uniform random within bounds (default, existing behaviour).
    #[default]
    Uniform,
    /// Weight spawns toward Berlin hotspots (built-in data).
    BerlinHotspots,
}

/// Resource holding weighted spawn cells for riders and drivers.
///
/// When populated, the spawner system should prefer these cells over
/// uniform random positions. Each cell has a relative weight; selection
/// uses weighted random sampling.
#[derive(Debug, Resource)]
pub struct SpawnWeighting {
    /// Weighted cells for rider spawns (e.g. dense residential areas, POIs).
    pub rider_cells: Vec<WeightedCell>,
    /// Weighted cells for driver spawns (e.g. taxi stands, transit hubs).
    pub driver_cells: Vec<WeightedCell>,
    /// Cumulative weight sums for fast sampling (rider).
    rider_cumulative: Vec<f64>,
    /// Cumulative weight sums for fast sampling (driver).
    driver_cumulative: Vec<f64>,
}

impl SpawnWeighting {
    /// Create an empty weighting (uniform random will be used).
    pub fn uniform() -> Self {
        Self {
            rider_cells: Vec::new(),
            driver_cells: Vec::new(),
            rider_cumulative: Vec::new(),
            driver_cumulative: Vec::new(),
        }
    }

    /// Create Berlin hotspot weighting using approximate POI locations.
    ///
    /// Rider hotspots: dense areas like Mitte, Kreuzberg, Friedrichshain,
    /// Alexanderplatz, Hauptbahnhof, etc.
    /// Driver hotspots: transit hubs, train stations, taxi stands.
    pub fn berlin_hotspots() -> Self {
        let rider_hotspots = vec![
            // Mitte (city center) - high demand
            (52.520, 13.405, 3.0),
            // Alexanderplatz
            (52.521, 13.413, 2.5),
            // Hauptbahnhof
            (52.525, 13.369, 2.5),
            // Kreuzberg
            (52.497, 13.391, 2.0),
            // Friedrichshain
            (52.516, 13.454, 2.0),
            // Prenzlauer Berg
            (52.538, 13.424, 1.8),
            // Charlottenburg
            (52.507, 13.304, 1.5),
            // Schöneberg
            (52.484, 13.353, 1.5),
            // Neukölln
            (52.477, 13.442, 1.5),
            // Wedding
            (52.549, 13.359, 1.2),
            // Potsdamer Platz
            (52.509, 13.376, 2.0),
            // Kurfürstendamm
            (52.502, 13.326, 1.8),
            // Tegel (airport area)
            (52.554, 13.292, 1.0),
            // Tempelhof
            (52.473, 13.401, 1.2),
            // Spandau
            (52.535, 13.197, 0.8),
        ];

        let driver_hotspots = vec![
            // Hauptbahnhof (main station)
            (52.525, 13.369, 3.0),
            // Alexanderplatz
            (52.521, 13.413, 2.5),
            // Zoo station
            (52.507, 13.332, 2.0),
            // Ostbahnhof
            (52.510, 13.434, 2.0),
            // Südkreuz
            (52.475, 13.365, 1.8),
            // Mitte area
            (52.520, 13.405, 2.0),
            // Potsdamer Platz
            (52.509, 13.376, 1.8),
            // Friedrichstraße
            (52.520, 13.387, 1.5),
            // Gesundbrunnen
            (52.549, 13.388, 1.2),
            // Spandau station
            (52.534, 13.198, 0.8),
        ];

        fn cells_from_coords(data: &[(f64, f64, f64)]) -> (Vec<WeightedCell>, Vec<f64>) {
            let mut cells = Vec::new();
            let mut cumulative = Vec::new();
            let mut total = 0.0;
            for &(lat, lng, weight) in data {
                if let Ok(ll) = LatLng::new(lat, lng) {
                    let cell = ll.to_cell(Resolution::Nine);
                    cells.push(WeightedCell { cell, weight });
                    total += weight;
                    cumulative.push(total);
                }
            }
            (cells, cumulative)
        }

        let (rider_cells, rider_cumulative) = cells_from_coords(&rider_hotspots);
        let (driver_cells, driver_cumulative) = cells_from_coords(&driver_hotspots);

        Self {
            rider_cells,
            driver_cells,
            rider_cumulative,
            driver_cumulative,
        }
    }

    /// Build from a [`SpawnWeightingKind`] descriptor.
    pub fn from_kind(kind: &SpawnWeightingKind) -> Self {
        match kind {
            SpawnWeightingKind::Uniform => Self::uniform(),
            SpawnWeightingKind::BerlinHotspots => Self::berlin_hotspots(),
        }
    }

    /// Select a random rider cell using weighted sampling.
    /// Returns `None` if no weighted cells are available (use uniform fallback).
    pub fn sample_rider_cell<R: rand::Rng>(&self, rng: &mut R) -> Option<CellIndex> {
        self.sample_from(&self.rider_cells, &self.rider_cumulative, rng)
    }

    /// Select a random driver cell using weighted sampling.
    /// Returns `None` if no weighted cells are available (use uniform fallback).
    pub fn sample_driver_cell<R: rand::Rng>(&self, rng: &mut R) -> Option<CellIndex> {
        self.sample_from(&self.driver_cells, &self.driver_cumulative, rng)
    }

    fn sample_from<R: rand::Rng>(
        &self,
        cells: &[WeightedCell],
        cumulative: &[f64],
        rng: &mut R,
    ) -> Option<CellIndex> {
        if cells.is_empty() || cumulative.is_empty() {
            return None;
        }
        let total = *cumulative.last()?;
        if total <= 0.0 {
            return None;
        }
        let r: f64 = rng.gen_range(0.0..total);
        let idx = cumulative.partition_point(|&w| w <= r).min(cells.len() - 1);
        Some(cells[idx].cell)
    }
}
