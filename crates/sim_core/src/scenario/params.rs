use bevy_ecs::prelude::Resource;

use crate::pricing::PricingConfig;
use crate::routing::RouteProviderKind;
use crate::spawner::SpawnWeightingKind;
use crate::traffic::TrafficProfileKind;

/// Default bounding box: Berlin, Germany (approx).
const DEFAULT_LAT_MIN: f64 = 52.34;
const DEFAULT_LAT_MAX: f64 = 52.68;
const DEFAULT_LNG_MIN: f64 = 13.08;
const DEFAULT_LNG_MAX: f64 = 13.76;

/// Default time window for rider requests: 1 hour (simulation ms).
const DEFAULT_REQUEST_WINDOW_MS: u64 = 60 * 60 * 1000;

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

/// Parameters for building a simulation scenario.
#[derive(Debug, Clone)]
pub struct ScenarioParams {
    pub num_riders: usize,
    pub num_drivers: usize,
    pub initial_rider_count: usize,
    pub initial_driver_count: usize,
    pub seed: Option<u64>,
    pub lat_min: f64,
    pub lat_max: f64,
    pub lng_min: f64,
    pub lng_max: f64,
    pub request_window_ms: u64,
    pub driver_spread_ms: u64,
    pub match_radius: u32,
    pub min_trip_cells: u32,
    pub max_trip_cells: u32,
    /// Optional epoch for time-of-day patterns. If None, defaults to 0.
    pub epoch_ms: Option<i64>,
    /// Optional pricing configuration. If None, defaults are used.
    pub pricing_config: Option<PricingConfig>,
    /// Optional rider quote behavior config. If None, defaults are used.
    pub rider_quote_config: Option<RiderQuoteConfig>,
    /// Optional driver decision behavior config. If None, defaults are used.
    pub driver_decision_config: Option<DriverDecisionConfig>,
    /// Optional simulation end time in ms. If set, runner stops when next event >= this time.
    pub simulation_end_time_ms: Option<u64>,
    /// Matching algorithm type to use. If None, defaults to Hungarian.
    pub matching_algorithm_type: Option<MatchingAlgorithmType>,
    /// Whether batch matching is enabled. If None, defaults to true.
    pub batch_matching_enabled: Option<bool>,
    /// Batch matching interval in seconds. If None, defaults to 5.
    pub batch_interval_secs: Option<u64>,
    /// ETA weight for cost-based and Hungarian matching algorithms. If None, uses DEFAULT_ETA_WEIGHT.
    pub eta_weight: Option<f64>,
    /// Which routing backend to use. Defaults to H3Grid (existing behaviour).
    pub route_provider_kind: RouteProviderKind,
    /// Traffic profile (time-of-day speed factors). Defaults to None (no traffic effects).
    pub traffic_profile: TrafficProfileKind,
    /// Whether spatial congestion zones are enabled.
    pub congestion_zones_enabled: bool,
    /// Whether dynamic congestion from vehicle density is enabled.
    pub dynamic_congestion_enabled: bool,
    /// Free-flow base speed in km/h (used as the reference speed before traffic factors).
    /// When set, overrides the default SpeedModel range. Defaults to None (use 20-60 km/h).
    pub base_speed_kmh: Option<f64>,
    /// Spawn location weighting. Defaults to Uniform (existing behaviour).
    pub spawn_weighting: SpawnWeightingKind,
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
            route_provider_kind: RouteProviderKind::default(),
            traffic_profile: TrafficProfileKind::default(),
            congestion_zones_enabled: false,
            dynamic_congestion_enabled: false,
            base_speed_kmh: None,
            spawn_weighting: SpawnWeightingKind::default(),
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
