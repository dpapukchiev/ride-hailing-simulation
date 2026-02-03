//! Application state and core simulation logic for the UI.

use bevy_ecs::prelude::World;
use std::time::Instant;

use sim_core::matching::{DEFAULT_ETA_WEIGHT, MatchingAlgorithmResource};
use sim_core::runner::{run_next_event, simulation_schedule};
use sim_core::scenario::{build_scenario, create_cost_based_matching, create_simple_matching, ScenarioParams};

use crate::ui::utils::{apply_cancel_config, apply_snapshot_interval, bounds_from_km, datetime_to_unix_ms, km_to_cells};

/// Main application state for the simulation UI.
pub struct SimUiApp {
    pub world: World,
    pub schedule: bevy_ecs::schedule::Schedule,
    pub steps_executed: usize,
    pub auto_run: bool,
    pub started: bool,
    pub snapshot_interval_ms: u64,
    pub speed_multiplier: f64,
    pub sim_budget_ms: f64,
    pub last_frame_instant: Option<Instant>,
    pub num_riders: usize,
    pub num_drivers: usize,
    pub initial_rider_count: usize,
    pub initial_driver_count: usize,
    pub request_window_hours: u64,
    pub driver_spread_hours: u64,
    pub match_radius_km: f64,
    pub min_trip_km: f64,
    pub max_trip_km: f64,
    pub seed_enabled: bool,
    pub seed_value: u64,
    pub grid_enabled: bool,
    pub map_size_km: f64,
    pub rider_cancel_min_mins: u64,
    pub rider_cancel_max_mins: u64,
    pub show_riders: bool,
    pub show_drivers: bool,
    pub show_driver_stats: bool,
    pub matching_algorithm: MatchingAlgorithmType,
    pub matching_algorithm_changed: bool,
    pub start_year: i32,
    pub start_month: u32,
    pub start_day: u32,
    pub start_hour: u32,
    pub start_minute: u32,
}

/// Type of matching algorithm to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchingAlgorithmType {
    Simple,
    CostBased,
}

impl MatchingAlgorithmType {
    fn create_matching_algorithm(&self) -> MatchingAlgorithmResource {
        match self {
            MatchingAlgorithmType::Simple => create_simple_matching(),
            MatchingAlgorithmType::CostBased => create_cost_based_matching(DEFAULT_ETA_WEIGHT),
        }
    }
}

impl SimUiApp {
    /// Create a new application instance with default settings.
    pub fn new() -> Self {
        let num_riders = 80;
        let num_drivers = 50;
        let initial_rider_count = 0;
        let initial_driver_count = 10;
        let request_window_hours = 21;
        let driver_spread_hours = 21;
        let match_radius_km = 11.0;
        let min_trip_km = 1.0;
        let max_trip_km = 25.0;
        let map_size_km = 25.0;
        let rider_cancel_min_mins = 10;
        let rider_cancel_max_mins = 40;
        let seed_enabled = true;
        let seed_value = 123;
        let matching_algorithm = MatchingAlgorithmType::CostBased;
        
        // Default start time: 2026-02-03 20:12:00 UTC
        let year = 2026;
        let month = 2;
        let day = 3;
        let hour = 20;
        let minute = 12;
        let start_epoch_ms = datetime_to_unix_ms(year, month, day, hour, minute);

        let mut params = ScenarioParams {
            num_riders,
            num_drivers,
            ..Default::default()
        }
        .with_request_window_hours(request_window_hours)
        .with_match_radius(km_to_cells(match_radius_km))
        .with_trip_duration_cells(km_to_cells(min_trip_km), km_to_cells(max_trip_km))
        .with_epoch_ms(start_epoch_ms);
        if seed_enabled {
            params = params.with_seed(seed_value);
        }

        let mut world = World::new();
        build_scenario(&mut world, params);
        // Override the default algorithm with the selected one
        world.insert_resource(matching_algorithm.create_matching_algorithm());
        // Clock epoch is already set in build_scenario from params.epoch_ms
        sim_core::runner::initialize_simulation(&mut world);
        let schedule = simulation_schedule();

        Self {
            world,
            schedule,
            steps_executed: 0,
            auto_run: false,
            started: false,
            snapshot_interval_ms: 1000,
            speed_multiplier: 200.0,
            sim_budget_ms: 0.0,
            last_frame_instant: None,
            num_riders,
            num_drivers,
            initial_rider_count,
            initial_driver_count,
            request_window_hours,
            driver_spread_hours,
            match_radius_km,
            min_trip_km,
            max_trip_km,
            seed_enabled,
            seed_value,
            grid_enabled: false,
            map_size_km,
            rider_cancel_min_mins,
            rider_cancel_max_mins,
            show_riders: true,
            show_drivers: true,
            show_driver_stats: true,
            matching_algorithm,
            matching_algorithm_changed: false,
            start_year: year,
            start_month: month,
            start_day: day,
            start_hour: hour,
            start_minute: minute,
        }
    }

    /// Reset the simulation to initial state with current parameters.
    pub fn reset(&mut self) {
        let mut world = World::new();
        build_scenario(&mut world, self.current_params());
        // Override the default algorithm with the selected one
        world.insert_resource(self.create_matching_algorithm());
        apply_cancel_config(
            &mut world,
            self.rider_cancel_min_mins,
            self.rider_cancel_max_mins,
        );
        // Clock epoch is already set in build_scenario from params.epoch_ms
        apply_snapshot_interval(&mut world, self.snapshot_interval_ms);
        sim_core::runner::initialize_simulation(&mut world);
        self.world = world;
        // Note: Schedule creation is fast (just adding systems), so recreating is fine
        // The main performance bottleneck was in build_scenario (grid_disk generation)
        self.schedule = simulation_schedule();
        self.steps_executed = 0;
        self.auto_run = false;
        self.started = false;
        self.sim_budget_ms = 0.0;
        self.last_frame_instant = None;
        self.matching_algorithm_changed = false;
    }

    /// Start the simulation with current parameters.
    pub fn start_simulation(&mut self) {
        let mut world = World::new();
        build_scenario(&mut world, self.current_params());
        // Override the default algorithm with the selected one
        world.insert_resource(self.create_matching_algorithm());
        apply_cancel_config(
            &mut world,
            self.rider_cancel_min_mins,
            self.rider_cancel_max_mins,
        );
        // Clock epoch is already set in build_scenario from params.epoch_ms
        apply_snapshot_interval(&mut world, self.snapshot_interval_ms);
        sim_core::runner::initialize_simulation(&mut world);
        self.world = world;
        self.schedule = simulation_schedule();
        self.steps_executed = 0;
        self.started = true;
        self.auto_run = true;
        self.sim_budget_ms = 0.0;
        self.last_frame_instant = Some(Instant::now());
    }

    /// Create a matching algorithm resource from the current algorithm type.
    pub fn create_matching_algorithm(&self) -> MatchingAlgorithmResource {
        match self.matching_algorithm {
            MatchingAlgorithmType::Simple => create_simple_matching(),
            MatchingAlgorithmType::CostBased => create_cost_based_matching(DEFAULT_ETA_WEIGHT),
        }
    }

    /// Build scenario parameters from current UI state.
    pub fn current_params(&self) -> ScenarioParams {
        let start_epoch_ms = datetime_to_unix_ms(self.start_year, self.start_month, self.start_day, self.start_hour, self.start_minute);
        let mut params = ScenarioParams {
            num_riders: self.num_riders,
            num_drivers: self.num_drivers,
            initial_rider_count: self.initial_rider_count,
            initial_driver_count: self.initial_driver_count,
            ..Default::default()
        }
        .with_request_window_hours(self.request_window_hours)
        .with_driver_spread_hours(self.driver_spread_hours)
        .with_match_radius(km_to_cells(self.match_radius_km))
        .with_trip_duration_cells(km_to_cells(self.min_trip_km), km_to_cells(self.max_trip_km))
        .with_epoch_ms(start_epoch_ms);
        let (lat_min, lat_max, lng_min, lng_max) = bounds_from_km(self.map_size_km);
        params.lat_min = lat_min;
        params.lat_max = lat_max;
        params.lng_min = lng_min;
        params.lng_max = lng_max;

        if self.seed_enabled {
            params = params.with_seed(self.seed_value);
        }
        params
    }

    /// Run a specified number of simulation steps.
    pub fn run_steps(&mut self, steps: usize) {
        for _ in 0..steps {
            if !run_next_event(&mut self.world, &mut self.schedule) {
                break;
            }
            self.steps_executed += 1;
        }
    }

    /// Run simulation until all events are processed.
    pub fn run_until_done(&mut self) {
        loop {
            if !run_next_event(&mut self.world, &mut self.schedule) {
                break;
            }
            self.steps_executed += 1;
        }
    }

    /// Advance simulation by a time budget (in milliseconds).
    pub fn advance_by_budget(&mut self, budget_ms: f64) {
        let mut remaining = budget_ms.max(0.0);
        while let Some((next_ts, sim_now)) = self
            .world
            .get_resource::<sim_core::clock::SimulationClock>()
            .and_then(|clock| Some((clock.next_event_time()?, clock.now())))
        {
            if next_ts <= sim_now {
                if !run_next_event(&mut self.world, &mut self.schedule) {
                    break;
                }
                self.steps_executed += 1;
                continue;
            }

            let gap = (next_ts - sim_now) as f64;
            if gap > remaining {
                break;
            }
            if !run_next_event(&mut self.world, &mut self.schedule) {
                break;
            }
            self.steps_executed += 1;
            remaining -= gap;
        }
        self.sim_budget_ms = remaining;
    }
}
