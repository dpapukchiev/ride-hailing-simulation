use bevy_ecs::prelude::World;

use crate::clock::SimulationClock;
use crate::distributions::TimeOfDayDistribution;
use crate::matching::{
    CostBasedMatching, HungarianMatching, MatchingAlgorithmResource, SimpleMatching,
};
use crate::patterns::{apply_driver_patterns, apply_rider_patterns};
#[cfg(feature = "osrm")]
use crate::routing::osrm_spawn::OsrmSpawnClient;
#[cfg(feature = "osrm")]
use crate::routing::RouteProviderKind;
use crate::routing::{build_route_provider, RouteProviderResource};
use crate::scenario::params::{
    BatchMatchingConfig, DriverDecisionConfig, MatchRadius, MatchingAlgorithmType,
    RiderCancelConfig, RiderQuoteConfig, ScenarioParams, SimulationEndTimeMs,
};
use crate::spatial::{cell_in_bounds, GeoIndex, SpatialIndex};
use crate::spawner::{
    DriverSpawner, DriverSpawnerConfig, RiderSpawner, RiderSpawnerConfig, SpawnWeighting,
};
use crate::speed::SpeedModel;
#[cfg(feature = "osrm")]
use crate::telemetry::OsrmSpawnTelemetry;
use crate::telemetry::{SimSnapshotConfig, SimSnapshots, SimTelemetry};
use crate::traffic::{CongestionZones, DynamicCongestionConfig, TrafficProfile};

/// Average multiplier for rider demand patterns.
/// Used to adjust base spawn rate to account for time-of-day variations.
/// The actual multipliers vary by hour (rush hours ~2.5-3.2x, night ~0.3-0.4x),
/// but the average across all hours is approximately 1.3.
const RIDER_DEMAND_AVERAGE_MULTIPLIER: f64 = 1.3;

/// Average multiplier for driver supply patterns.
/// Used to adjust base spawn rate to account for time-of-day variations.
/// Driver supply is more consistent than demand, with an average multiplier of approximately 1.2.
const DRIVER_SUPPLY_AVERAGE_MULTIPLIER: f64 = 1.2;

/// Threshold for choosing between grid_disk and rejection sampling strategies.
/// For distances <= this threshold, grid_disk is more efficient.
/// For larger distances, rejection sampling avoids generating huge grid disks.
const GRID_DISK_THRESHOLD: u32 = 20;

/// Maximum attempts for rejection sampling before falling back to grid_disk.
const MAX_REJECTION_SAMPLING_ATTEMPTS: usize = 2000;

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
    geo: &GeoIndex,
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

#[allow(clippy::too_many_arguments)]
fn fallback_grid_disk_strategy<R: rand::Rng>(
    rng: &mut R,
    pickup: h3o::CellIndex,
    geo: &GeoIndex,
    min_cells: u32,
    max_cells: u32,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
) -> Option<h3o::CellIndex> {
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

#[allow(clippy::too_many_arguments)]
pub fn random_destination<R: rand::Rng>(
    rng: &mut R,
    pickup: h3o::CellIndex,
    geo: &GeoIndex,
    min_cells: u32,
    max_cells: u32,
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
) -> h3o::CellIndex {
    let max_cells = max_cells.max(min_cells);

    if max_cells <= GRID_DISK_THRESHOLD {
        if let Some(destination) = grid_disk_strategy(
            rng, pickup, geo, min_cells, max_cells, lat_min, lat_max, lng_min, lng_max,
        ) {
            return destination;
        }
    }

    if let Some(destination) = rejection_sampling_strategy(
        rng, pickup, min_cells, max_cells, lat_min, lat_max, lng_min, lng_max,
    ) {
        return destination;
    }

    if let Some(destination) = fallback_grid_disk_strategy(
        rng, pickup, geo, min_cells, max_cells, lat_min, lat_max, lng_min, lng_max,
    ) {
        return destination;
    }

    pickup
}

pub fn create_simple_matching() -> MatchingAlgorithmResource {
    MatchingAlgorithmResource::new(Box::new(SimpleMatching))
}

pub fn create_cost_based_matching(eta_weight: f64) -> MatchingAlgorithmResource {
    MatchingAlgorithmResource::new(Box::new(CostBasedMatching::new(eta_weight)))
}

pub fn create_hungarian_matching(eta_weight: f64) -> MatchingAlgorithmResource {
    MatchingAlgorithmResource::new(Box::new(HungarianMatching::new(eta_weight)))
}

fn create_rider_time_of_day_pattern(
    base_rate_per_sec: f64,
    epoch_ms: i64,
    seed: u64,
) -> TimeOfDayDistribution {
    let dist = TimeOfDayDistribution::new(base_rate_per_sec, epoch_ms, seed);
    apply_rider_patterns(dist)
}

fn create_driver_time_of_day_pattern(
    base_rate_per_sec: f64,
    epoch_ms: i64,
    seed: u64,
) -> TimeOfDayDistribution {
    let dist = TimeOfDayDistribution::new(base_rate_per_sec, epoch_ms, seed);
    apply_driver_patterns(dist)
}

pub fn build_scenario(world: &mut World, params: ScenarioParams) {
    let epoch_ms = params.epoch_ms.unwrap_or(0);
    let mut clock = SimulationClock::default();
    clock.set_epoch_ms(epoch_ms);
    world.insert_resource(clock);

    world.insert_resource(SimTelemetry::default());
    world.insert_resource(SimSnapshotConfig::default());
    world.insert_resource(SimSnapshots::default());

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
        seed: seed.wrapping_add(0xcafe_babe),
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
    if let Some(base) = params.base_speed_kmh {
        let min = (base - 10.0).max(5.0);
        let max = base + 10.0;
        world.insert_resource(SpeedModel::with_range(
            params.seed.map(|seed| seed ^ 0x5eed_cafe),
            min,
            max,
        ));
    } else {
        world.insert_resource(SpeedModel::new(params.seed.map(|seed| seed ^ 0x5eed_cafe)));
    }

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
    world.insert_resource(params.pricing_config.unwrap_or_default());

    let route_provider = build_route_provider(&params.route_provider_kind);
    world.insert_resource(RouteProviderResource(route_provider));

    #[cfg(feature = "osrm")]
    let osrm_spawn_client = match &params.route_provider_kind {
        RouteProviderKind::Osrm { endpoint } => Some(OsrmSpawnClient::new(endpoint)),
        _ => None,
    };

    #[cfg(feature = "osrm")]
    if matches!(params.route_provider_kind, RouteProviderKind::Osrm { .. }) {
        world.insert_resource(OsrmSpawnTelemetry::default());
    }

    world.insert_resource(TrafficProfile::from_kind(&params.traffic_profile));
    if params.congestion_zones_enabled {
        world.insert_resource(CongestionZones::berlin_defaults());
    } else {
        world.insert_resource(CongestionZones::default());
    }
    world.insert_resource(DynamicCongestionConfig {
        enabled: params.dynamic_congestion_enabled,
    });

    world.insert_resource(SpawnWeighting::from_kind(&params.spawn_weighting));

    let request_window_ms = params.request_window_ms;
    let driver_spread_ms = params.driver_spread_ms;
    let lat_min = params.lat_min;
    let lat_max = params.lat_max;
    let lng_min = params.lng_min;
    let lng_max = params.lng_max;
    let min_trip = params.min_trip_cells;
    let max_trip = params.max_trip_cells;

    let scheduled_rider_count = params.num_riders.saturating_sub(params.initial_rider_count);
    let avg_rate_per_sec = if request_window_ms > 0 && scheduled_rider_count > 0 {
        (scheduled_rider_count as f64) / (request_window_ms as f64 / 1000.0)
    } else {
        0.0
    };
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
    let rider_spawner = {
        let base = RiderSpawner::new(rider_spawner_config);
        #[cfg(feature = "osrm")]
        {
            base.with_osrm_spawn_client(osrm_spawn_client.clone())
        }
        #[cfg(not(feature = "osrm"))]
        {
            base
        }
    };
    world.insert_resource(rider_spawner);

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
    let driver_spawner = {
        let base = DriverSpawner::new(driver_spawner_config);
        #[cfg(feature = "osrm")]
        {
            base.with_osrm_spawn_client(osrm_spawn_client.clone())
        }
        #[cfg(not(feature = "osrm"))]
        {
            base
        }
    };
    world.insert_resource(driver_spawner);
}
