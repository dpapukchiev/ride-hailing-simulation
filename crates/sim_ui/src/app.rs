//! Application state and core simulation logic for the UI.

use bevy_ecs::prelude::World;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use sim_core::matching::{MatchingAlgorithmResource, DEFAULT_ETA_WEIGHT};
use sim_core::pricing::PricingConfig;
use sim_core::routing::RouteProviderKind;
use sim_core::runner::{run_next_event, simulation_schedule};
use sim_core::scenario::{
    build_scenario, create_cost_based_matching, create_hungarian_matching, create_simple_matching,
    DriverDecisionConfig, RiderQuoteConfig, ScenarioParams,
};
use sim_core::spawner::SpawnWeightingKind;
use sim_core::traffic::TrafficProfileKind;

use crate::ui::utils::{
    apply_batch_config, apply_cancel_config, apply_snapshot_interval, bounds_from_km,
    datetime_to_unix_ms, km_to_cells,
};

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
    /// Simulation stops when clock reaches this time (hours of sim time).
    pub simulation_duration_hours: u64,
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
    pub hide_off_duty_drivers: bool,
    pub matching_algorithm: MatchingAlgorithmType,
    pub matching_algorithm_changed: bool,
    pub batch_matching_enabled: bool,
    pub batch_interval_secs: u64,
    pub start_year: i32,
    pub start_month: u32,
    pub start_day: u32,
    pub start_hour: u32,
    pub start_minute: u32,
    pub base_fare: f64,
    pub per_km_rate: f64,
    pub commission_rate: f64,
    pub surge_enabled: bool,
    pub surge_radius_k: u32,
    pub surge_max_multiplier: f64,
    pub max_willingness_to_pay: f64,
    pub max_acceptable_eta_min: u64,
    pub accept_probability: f64,
    pub max_quote_rejections: u32,
    pub driver_base_acceptance_score: f64,
    pub driver_fare_weight: f64,
    pub driver_pickup_distance_penalty: f64,
    // Routing & Traffic
    pub routing_mode: RoutingMode,
    pub osrm_endpoint: String,
    pub traffic_profile_mode: TrafficProfileMode,
    pub congestion_zones_enabled: bool,
    pub dynamic_congestion_enabled: bool,
    pub base_speed_enabled: bool,
    pub base_speed_kmh: f64,
    pub spawn_mode: SpawnMode,
    pub map_tiles: MapTileState,
}

/// Type of matching algorithm to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchingAlgorithmType {
    Simple,
    CostBased,
    Hungarian,
}

/// Routing backend to use (UI-friendly enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingMode {
    H3Grid,
    Osrm,
}

/// Traffic profile to apply (UI-friendly enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrafficProfileMode {
    None,
    Berlin,
}

/// Spawn location weighting (UI-friendly enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnMode {
    Uniform,
    BerlinHotspots,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileKey {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MapSignature {
    pub z: u8,
    pub lat_min: i64,
    pub lat_max: i64,
    pub lng_min: i64,
    pub lng_max: i64,
}

#[derive(Clone, Copy)]
struct ProjectionBounds {
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
}

impl ProjectionBounds {
    fn new(lat_min: f64, lat_max: f64, lng_min: f64, lng_max: f64) -> Option<Self> {
        if lat_max > lat_min && lng_max > lng_min {
            Some(Self {
                lat_min,
                lat_max,
                lng_min,
                lng_max,
            })
        } else {
            None
        }
    }

    fn lat_span(&self) -> f64 {
        self.lat_max - self.lat_min
    }

    fn lng_span(&self) -> f64 {
        self.lng_max - self.lng_min
    }
}

struct CachedProjection {
    normalized_lines: Vec<Vec<(f32, f32)>>,
    last_used: Instant,
}

pub struct MapTileState {
    tiles: HashMap<TileKey, TileGeometry>,
    inflight: HashSet<TileKey>,
    errors: HashMap<TileKey, String>,
    sender: Sender<TileResult>,
    receiver: Receiver<TileResult>,
    last_signature: Option<MapSignature>,
    current_projection_bounds: Option<ProjectionBounds>,
    projection_cache: HashMap<TileKey, CachedProjection>,
}

#[derive(Debug, Clone)]
pub struct TileGeometry {
    pub lines: Vec<Vec<(f64, f64)>>,
}

struct TileResult {
    key: TileKey,
    geometry: Option<TileGeometry>,
    error: Option<String>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
struct VectorTile {
    #[prost(message, repeated, tag = "3")]
    pub layers: Vec<VectorTileLayer>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
struct VectorTileLayer {
    #[prost(uint32, tag = "15")]
    pub version: u32,
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(message, repeated, tag = "2")]
    pub features: Vec<VectorTileFeature>,
    #[prost(uint32, tag = "5")]
    pub extent: u32,
}

#[derive(Clone, PartialEq, ::prost::Message)]
struct VectorTileFeature {
    #[prost(uint64, tag = "1")]
    pub id: u64,
    #[prost(uint32, repeated, packed = "true", tag = "2")]
    pub tags: Vec<u32>,
    #[prost(enumeration = "GeomType", tag = "3")]
    pub r#type: i32,
    #[prost(uint32, repeated, packed = "true", tag = "4")]
    pub geometry: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ::prost::Enumeration)]
#[repr(i32)]
enum GeomType {
    Unknown = 0,
    Point = 1,
    Linestring = 2,
    Polygon = 3,
}

impl MapTileState {
    pub fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        Self {
            tiles: HashMap::new(),
            inflight: HashSet::new(),
            errors: HashMap::new(),
            sender,
            receiver,
            last_signature: None,
            current_projection_bounds: None,
            projection_cache: HashMap::new(),
        }
    }

    pub fn update_signature(&mut self, signature: MapSignature) {
        if self.last_signature == Some(signature) {
            return;
        }
        self.tiles.clear();
        self.inflight.clear();
        self.errors.clear();
        self.current_projection_bounds = ProjectionBounds::new(
            signature.lat_min as f64 / 1_000_000.0,
            signature.lat_max as f64 / 1_000_000.0,
            signature.lng_min as f64 / 1_000_000.0,
            signature.lng_max as f64 / 1_000_000.0,
        );
        self.projection_cache.clear();
        self.last_signature = Some(signature);
    }

    pub fn drain_results(&mut self) {
        while let Ok(result) = self.receiver.try_recv() {
            self.inflight.remove(&result.key);
            if let Some(error) = result.error {
                self.errors.insert(result.key, error);
                continue;
            }
            if let Some(geometry) = result.geometry {
                self.cache_projection_from_geometry(result.key, &geometry);
                self.tiles.insert(result.key, geometry);
            }
        }
    }

    fn current_inflight_limit(&self) -> usize {
        const WARMUP_TILES: usize = 6;
        const WARMUP_LIMIT: usize = 4;
        const MAX_LIMIT: usize = 12;
        if self.tiles.len() >= WARMUP_TILES {
            MAX_LIMIT
        } else {
            WARMUP_LIMIT
        }
    }

    pub fn projected(&self, key: &TileKey) -> Option<&CachedProjection> {
        self.projection_cache.get(key)
    }

    pub fn touch_projection(&mut self, key: &TileKey) {
        if let Some(entry) = self.projection_cache.get_mut(key) {
            entry.last_used = Instant::now();
        }
    }

    pub fn cache_projection_from_geometry(&mut self, key: TileKey, geometry: &TileGeometry) {
        let bounds = match self.current_projection_bounds {
            Some(bounds) => bounds,
            None => return,
        };
        let lat_span = bounds.lat_span();
        let lng_span = bounds.lng_span();
        if lat_span <= 0.0 || lng_span <= 0.0 {
            return;
        }

        const TOLERANCE: f32 = 0.002;
        let mut normalized_lines = Vec::new();
        for line in &geometry.lines {
            let mut projected = Vec::new();
            let mut last_point: Option<(f32, f32)> = None;
            for &(lat, lng) in line {
                let mut x = ((lng - bounds.lng_min) / lng_span) as f32;
                let mut y = ((bounds.lat_max - lat) / lat_span) as f32;
                x = x.clamp(0.0, 1.0);
                y = y.clamp(0.0, 1.0);
                let point = (x, y);
                if let Some(last) = last_point {
                    if (point.0 - last.0).abs() < TOLERANCE && (point.1 - last.1).abs() < TOLERANCE
                    {
                        continue;
                    }
                }
                projected.push(point);
                last_point = Some(point);
            }
            if projected.len() >= 2 {
                normalized_lines.push(projected);
            }
        }
        if normalized_lines.is_empty() {
            return;
        }
        self.projection_cache.insert(
            key,
            CachedProjection {
                normalized_lines,
                last_used: Instant::now(),
            },
        );
    }

    pub fn evict_stale_projections(&mut self) {
        let now = Instant::now();
        let ttl = Duration::from_secs(5);
        self.projection_cache
            .retain(|_, entry| now.duration_since(entry.last_used) <= ttl);
    }

    pub fn request_missing_tiles<I>(&mut self, endpoint: &str, keys: I)
    where
        I: IntoIterator<Item = TileKey>,
    {
        let mut inflight_count = self.inflight.len();
        let endpoint = endpoint.trim_end_matches('/').to_string();
        for key in keys {
            if self.tiles.contains_key(&key) || self.inflight.contains(&key) {
                continue;
            }
            if self.errors.contains_key(&key) {
                continue;
            }
            let limit = self.current_inflight_limit();
            if inflight_count >= limit {
                break;
            }
            inflight_count += 1;
            self.inflight.insert(key);
            let sender = self.sender.clone();
            let url = format!(
                "{}/tile/v1/driving/tile({},{},{}).mvt",
                endpoint, key.x, key.y, key.z
            );
            std::thread::spawn(move || {
                let result = fetch_tile(&url, key);
                let _ = sender.send(result);
            });
        }
    }

    pub fn tile(&self, key: &TileKey) -> Option<&TileGeometry> {
        self.tiles.get(key)
    }
}

fn fetch_tile(url: &str, key: TileKey) -> TileResult {
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            return TileResult {
                key,
                geometry: None,
                error: Some(err.to_string()),
            };
        }
    };
    let response = match client.get(url).send() {
        Ok(response) => response,
        Err(err) => {
            return TileResult {
                key,
                geometry: None,
                error: Some(err.to_string()),
            };
        }
    };
    if !response.status().is_success() {
        return TileResult {
            key,
            geometry: None,
            error: Some(format!("status {}", response.status())),
        };
    }
    let bytes = match response.bytes() {
        Ok(bytes) => bytes,
        Err(err) => {
            return TileResult {
                key,
                geometry: None,
                error: Some(err.to_string()),
            };
        }
    };
    match decode_tile_geometry(key, bytes.to_vec()) {
        Ok(geometry) => TileResult {
            key,
            geometry: Some(geometry),
            error: None,
        },
        Err(err) => TileResult {
            key,
            geometry: None,
            error: Some(err),
        },
    }
}

fn decode_tile_geometry(key: TileKey, data: Vec<u8>) -> Result<TileGeometry, String> {
    use prost::Message;

    let tile = VectorTile::decode(data.as_slice()).map_err(|err| err.to_string())?;
    let layer = match tile.layers.iter().find(|layer| layer.name == "speeds") {
        Some(layer) => layer,
        None => return Ok(TileGeometry { lines: Vec::new() }),
    };
    let extent = if layer.extent == 0 {
        4096.0
    } else {
        layer.extent as f64
    };
    let mut lines = Vec::new();
    for feature in &layer.features {
        if feature.r#type != GeomType::Linestring as i32 {
            continue;
        }
        let tile_lines = decode_line_strings(&feature.geometry);
        for line in tile_lines {
            let points = line
                .into_iter()
                .map(|(x, y)| tile_point_to_lat_lng(key, x as f64, y as f64, extent))
                .collect();
            lines.push(points);
        }
    }
    Ok(TileGeometry { lines })
}

fn decode_line_strings(geometry: &[u32]) -> Vec<Vec<(i32, i32)>> {
    let mut lines: Vec<Vec<(i32, i32)>> = Vec::new();
    let mut cursor = 0usize;
    let mut x = 0i32;
    let mut y = 0i32;
    while cursor < geometry.len() {
        let command = geometry[cursor];
        cursor += 1;
        let id = command & 0x7;
        let count = command >> 3;
        match id {
            1 => {
                for _ in 0..count {
                    if cursor + 1 >= geometry.len() {
                        break;
                    }
                    x += decode_zigzag(geometry[cursor]);
                    y += decode_zigzag(geometry[cursor + 1]);
                    cursor += 2;
                    lines.push(vec![(x, y)]);
                }
            }
            2 => {
                for _ in 0..count {
                    if cursor + 1 >= geometry.len() {
                        break;
                    }
                    x += decode_zigzag(geometry[cursor]);
                    y += decode_zigzag(geometry[cursor + 1]);
                    cursor += 2;
                    if let Some(current) = lines.last_mut() {
                        current.push((x, y));
                    }
                }
            }
            7 => {}
            _ => break,
        }
    }
    lines
}

fn decode_zigzag(value: u32) -> i32 {
    ((value >> 1) as i32) ^ (-((value & 1) as i32))
}

fn tile_point_to_lat_lng(key: TileKey, x: f64, y: f64, extent: f64) -> (f64, f64) {
    let n = (1u32 << key.z) as f64;
    let gx = (key.x as f64 + (x / extent)) / n;
    let gy = (key.y as f64 + (y / extent)) / n;
    let lng = gx * 360.0 - 180.0;
    let lat = (std::f64::consts::PI * (1.0 - 2.0 * gy))
        .sinh()
        .atan()
        .to_degrees();
    (lat, lng)
}

impl MatchingAlgorithmType {
    fn create_matching_algorithm(&self) -> MatchingAlgorithmResource {
        match self {
            MatchingAlgorithmType::Simple => create_simple_matching(),
            MatchingAlgorithmType::CostBased => create_cost_based_matching(DEFAULT_ETA_WEIGHT),
            MatchingAlgorithmType::Hungarian => create_hungarian_matching(DEFAULT_ETA_WEIGHT),
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
        let simulation_duration_hours = 24;
        let match_radius_km = 11.0;
        let min_trip_km = 1.0;
        let max_trip_km = 25.0;
        let map_size_km = 25.0;
        let rider_cancel_min_mins = 6;
        let rider_cancel_max_mins = 40;
        let seed_enabled = true;
        let seed_value = 123;
        let matching_algorithm = MatchingAlgorithmType::Hungarian;
        let batch_matching_enabled = true;
        let batch_interval_secs = 20;
        let base_fare = 1.20;
        let per_km_rate = 1.00;
        let commission_rate = 0.175;
        let surge_enabled = true;
        let surge_radius_k = 2;
        let surge_max_multiplier = 2.0;
        let max_willingness_to_pay = 50.0;
        let max_acceptable_eta_min: u64 = 20;
        let accept_probability = 0.8;
        let max_quote_rejections = 3;
        let driver_base_acceptance_score = 1.0;
        let driver_fare_weight = 0.37;
        let driver_pickup_distance_penalty = -0.7;
        let routing_mode = RoutingMode::H3Grid;
        let osrm_endpoint = "http://localhost:5000".to_string();
        let traffic_profile_mode = TrafficProfileMode::None;
        let congestion_zones_enabled = false;
        let dynamic_congestion_enabled = false;
        let base_speed_enabled = false;
        let base_speed_kmh = 50.0;
        let spawn_mode = SpawnMode::Uniform;
        let map_tiles = MapTileState::new();

        // Default start time: 2026-02-03 06:30:00 UTC
        let year = 2026;
        let month = 2;
        let day = 3;
        let hour = 6;
        let minute = 30;
        let start_epoch_ms = datetime_to_unix_ms(year, month, day, hour, minute);

        let mut params = ScenarioParams {
            num_riders,
            num_drivers,
            ..Default::default()
        }
        .with_request_window_hours(request_window_hours)
        .with_match_radius(km_to_cells(match_radius_km))
        .with_trip_duration_cells(km_to_cells(min_trip_km), km_to_cells(max_trip_km))
        .with_epoch_ms(start_epoch_ms)
        .with_pricing_config(PricingConfig {
            base_fare,
            per_km_rate,
            commission_rate,
            surge_enabled,
            surge_radius_k,
            surge_max_multiplier,
        })
        .with_rider_quote_config(RiderQuoteConfig {
            max_quote_rejections,
            re_quote_delay_secs: 10,
            accept_probability,
            seed: if seed_enabled { seed_value } else { 0u64 }.wrapping_add(0x0071_1073_beef_u64),
            max_willingness_to_pay,
            max_acceptable_eta_ms: max_acceptable_eta_min
                .saturating_mul(60)
                .saturating_mul(1000),
        })
        .with_driver_decision_config(DriverDecisionConfig {
            seed: if seed_enabled { seed_value } else { 0u64 }.wrapping_add(0xdead_beef_u64),
            base_acceptance_score: driver_base_acceptance_score,
            fare_weight: driver_fare_weight,
            pickup_distance_penalty: driver_pickup_distance_penalty,
            ..Default::default()
        });
        if seed_enabled {
            params = params.with_seed(seed_value);
        }

        let mut world = World::new();
        build_scenario(&mut world, params);
        // Override the default algorithm and batch config with the selected ones
        world.insert_resource(matching_algorithm.create_matching_algorithm());
        apply_batch_config(&mut world, batch_matching_enabled, batch_interval_secs);
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
            speed_multiplier: 1000.0,
            sim_budget_ms: 0.0,
            last_frame_instant: None,
            num_riders,
            num_drivers,
            initial_rider_count,
            initial_driver_count,
            request_window_hours,
            driver_spread_hours,
            simulation_duration_hours,
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
            hide_off_duty_drivers: true,
            matching_algorithm,
            matching_algorithm_changed: false,
            batch_matching_enabled,
            batch_interval_secs,
            start_year: year,
            start_month: month,
            start_day: day,
            start_hour: hour,
            start_minute: minute,
            base_fare,
            per_km_rate,
            commission_rate,
            surge_enabled,
            surge_radius_k,
            surge_max_multiplier,
            max_willingness_to_pay,
            max_acceptable_eta_min,
            accept_probability,
            max_quote_rejections,
            driver_base_acceptance_score,
            driver_fare_weight,
            driver_pickup_distance_penalty,
            routing_mode,
            osrm_endpoint,
            traffic_profile_mode,
            congestion_zones_enabled,
            dynamic_congestion_enabled,
            base_speed_enabled,
            base_speed_kmh,
            spawn_mode,
            map_tiles,
        }
    }

    /// Reset the simulation to initial state with current parameters.
    pub fn reset(&mut self) {
        let mut world = World::new();
        build_scenario(&mut world, self.current_params());
        // Override the default algorithm and batch config with the selected ones
        world.insert_resource(self.create_matching_algorithm());
        apply_batch_config(
            &mut world,
            self.batch_matching_enabled,
            self.batch_interval_secs,
        );
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
        // Override the default algorithm and batch config with the selected ones
        world.insert_resource(self.create_matching_algorithm());
        apply_batch_config(
            &mut world,
            self.batch_matching_enabled,
            self.batch_interval_secs,
        );
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
            MatchingAlgorithmType::Hungarian => create_hungarian_matching(DEFAULT_ETA_WEIGHT),
        }
    }

    /// Build scenario parameters from current UI state.
    pub fn current_params(&self) -> ScenarioParams {
        let start_epoch_ms = datetime_to_unix_ms(
            self.start_year,
            self.start_month,
            self.start_day,
            self.start_hour,
            self.start_minute,
        );
        let mut params = ScenarioParams {
            num_riders: self.num_riders,
            num_drivers: self.num_drivers,
            initial_rider_count: self.initial_rider_count,
            initial_driver_count: self.initial_driver_count,
            ..Default::default()
        }
        .with_request_window_hours(self.request_window_hours)
        .with_driver_spread_hours(self.driver_spread_hours)
        .with_simulation_end_time_ms(self.simulation_duration_hours * 3600 * 1000)
        .with_match_radius(km_to_cells(self.match_radius_km))
        .with_trip_duration_cells(km_to_cells(self.min_trip_km), km_to_cells(self.max_trip_km))
        .with_epoch_ms(start_epoch_ms);
        let (lat_min, lat_max, lng_min, lng_max) = bounds_from_km(self.map_size_km);
        params.lat_min = lat_min;
        params.lat_max = lat_max;
        params.lng_min = lng_min;
        params.lng_max = lng_max;

        params = params
            .with_pricing_config(PricingConfig {
                base_fare: self.base_fare,
                per_km_rate: self.per_km_rate,
                commission_rate: self.commission_rate,
                surge_enabled: self.surge_enabled,
                surge_radius_k: self.surge_radius_k,
                surge_max_multiplier: self.surge_max_multiplier,
            })
            .with_rider_quote_config(RiderQuoteConfig {
                max_quote_rejections: self.max_quote_rejections,
                re_quote_delay_secs: 10,
                accept_probability: self.accept_probability,
                seed: if self.seed_enabled {
                    self.seed_value
                } else {
                    0u64
                }
                .wrapping_add(0x0071_1073_beef_u64),
                max_willingness_to_pay: self.max_willingness_to_pay,
                max_acceptable_eta_ms: self
                    .max_acceptable_eta_min
                    .saturating_mul(60)
                    .saturating_mul(1000),
            })
            .with_driver_decision_config(DriverDecisionConfig {
                seed: if self.seed_enabled {
                    self.seed_value
                } else {
                    0u64
                }
                .wrapping_add(0xdead_beef_u64),
                base_acceptance_score: self.driver_base_acceptance_score,
                fare_weight: self.driver_fare_weight,
                pickup_distance_penalty: self.driver_pickup_distance_penalty,
                ..Default::default()
            });

        if self.seed_enabled {
            params = params.with_seed(self.seed_value);
        }

        // Routing & traffic
        params.route_provider_kind = match self.routing_mode {
            RoutingMode::H3Grid => RouteProviderKind::H3Grid,
            RoutingMode::Osrm => RouteProviderKind::Osrm {
                endpoint: self.osrm_endpoint.clone(),
            },
        };
        params.traffic_profile = match self.traffic_profile_mode {
            TrafficProfileMode::None => TrafficProfileKind::None,
            TrafficProfileMode::Berlin => TrafficProfileKind::Berlin,
        };
        params.congestion_zones_enabled = self.congestion_zones_enabled;
        params.dynamic_congestion_enabled = self.dynamic_congestion_enabled;
        params.base_speed_kmh = if self.base_speed_enabled {
            Some(self.base_speed_kmh)
        } else {
            None
        };
        params.spawn_weighting = match self.spawn_mode {
            SpawnMode::Uniform => SpawnWeightingKind::Uniform,
            SpawnMode::BerlinHotspots => SpawnWeightingKind::BerlinHotspots,
        };
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
