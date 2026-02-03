//! Entity spawners: dynamically spawn riders and drivers based on distributions.
//!
//! Spawners use inter-arrival time distributions to control spawn rates, enabling
//! variable supply and demand patterns. They react to SimulationStarted events
//! and schedule their own spawn events.

use bevy_ecs::prelude::Resource;
use h3o::{CellIndex, LatLng, Resolution};

use crate::distributions::InterArrivalDistribution;

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
    /// Next spawn time in simulation ms.
    pub next_spawn_time_ms: u64,
    /// Number of riders spawned so far.
    pub spawned_count: usize,
    /// Whether spawning has been initialized.
    pub initialized: bool,
}

impl RiderSpawner {
    /// Create a new rider spawner from configuration.
    pub fn new(config: RiderSpawnerConfig) -> Self {
        let start_time = config.start_time_ms.unwrap_or(0);
        Self {
            next_spawn_time_ms: start_time,
            spawned_count: 0,
            initialized: false,
            config,
        }
    }

    /// Check if the spawner should continue spawning at the current time.
    pub fn should_spawn(&self, current_time_ms: u64) -> bool {
        // Check if we've hit max count
        if let Some(max) = self.config.max_count {
            if self.spawned_count >= max {
                return false;
            }
        }

        // Check if we're past end time
        if let Some(end_time) = self.config.end_time_ms {
            if current_time_ms > end_time {
                return false;
            }
        }

        // Check if we're before start time
        if let Some(start_time) = self.config.start_time_ms {
            if current_time_ms < start_time {
                return false;
            }
        }

        // Check if it's time to spawn
        current_time_ms >= self.next_spawn_time_ms
    }

    /// Advance the spawner to the next spawn time.
    /// Returns the inter-arrival time that was sampled (in ms).
    pub fn advance(&mut self, current_time_ms: u64) -> f64 {
        let inter_arrival_ms = self.config.inter_arrival_dist.sample_ms(self.spawned_count as u64, current_time_ms);
        self.next_spawn_time_ms = current_time_ms + inter_arrival_ms.max(0.0) as u64;
        self.spawned_count += 1;
        inter_arrival_ms
    }
}

/// Active driver spawner resource that tracks spawning state.
#[derive(Debug, Resource)]
pub struct DriverSpawner {
    pub config: DriverSpawnerConfig,
    /// Next spawn time in simulation ms.
    pub next_spawn_time_ms: u64,
    /// Number of drivers spawned so far.
    pub spawned_count: usize,
    /// Whether spawning has been initialized.
    pub initialized: bool,
}

impl DriverSpawner {
    /// Create a new driver spawner from configuration.
    pub fn new(config: DriverSpawnerConfig) -> Self {
        let start_time = config.start_time_ms.unwrap_or(0);
        Self {
            next_spawn_time_ms: start_time,
            spawned_count: 0,
            initialized: false,
            config,
        }
    }

    /// Check if the spawner should continue spawning at the current time.
    pub fn should_spawn(&self, current_time_ms: u64) -> bool {
        // Check if we've hit max count
        if let Some(max) = self.config.max_count {
            if self.spawned_count >= max {
                return false;
            }
        }

        // Check if we're past end time
        if let Some(end_time) = self.config.end_time_ms {
            if current_time_ms > end_time {
                return false;
            }
        }

        // Check if we're before start time
        if let Some(start_time) = self.config.start_time_ms {
            if current_time_ms < start_time {
                return false;
            }
        }

        // Check if it's time to spawn
        current_time_ms >= self.next_spawn_time_ms
    }

    /// Advance the spawner to the next spawn time.
    /// Returns the inter-arrival time that was sampled (in ms).
    pub fn advance(&mut self, current_time_ms: u64) -> f64 {
        let inter_arrival_ms = self.config.inter_arrival_dist.sample_ms(self.spawned_count as u64, current_time_ms);
        self.next_spawn_time_ms = current_time_ms + inter_arrival_ms.max(0.0) as u64;
        self.spawned_count += 1;
        inter_arrival_ms
    }
}

/// Sample a random H3 cell (resolution 9) within the given lat/lng bounds.
pub fn random_cell_in_bounds<R: rand::Rng>(rng: &mut R, lat_min: f64, lat_max: f64, lng_min: f64, lng_max: f64) -> CellIndex {
    let lat = rng.gen_range(lat_min..=lat_max);
    let lng = rng.gen_range(lng_min..=lng_max);
    let coord = LatLng::new(lat, lng).expect("valid lat/lng");
    coord.to_cell(Resolution::Nine)
}
