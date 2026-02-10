use bevy_ecs::prelude::World;
use std::time::Instant;

use sim_core::matching::{MatchingAlgorithmResource, DEFAULT_ETA_WEIGHT};
use sim_core::pricing::PricingConfig;
use sim_core::routing::RouteProviderKind;
use sim_core::runner::{run_next_event, simulation_schedule};
use sim_core::scenario::{
    build_scenario, create_cost_based_matching, create_hungarian_matching, create_simple_matching,
    DriverDecisionConfig, RepositionPolicyConfig, RiderQuoteConfig, ScenarioParams,
};
use sim_core::spawner::SpawnWeightingKind;
use sim_core::traffic::TrafficProfileKind;

use crate::app::defaults::AppDefaults;
use crate::app::map_tiles::MapTileState;
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
    pub reposition_control_interval_secs: u64,
    pub hotspot_weight: f64,
    pub minimum_zone_reserve: usize,
    pub reposition_cooldown_secs: u64,
    pub max_reposition_distance_km: f64,
    pub max_drivers_moved_per_cycle: usize,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchingAlgorithmType {
    Simple,
    CostBased,
    Hungarian,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingMode {
    H3Grid,
    Osrm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrafficProfileMode {
    None,
    Berlin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnMode {
    Uniform,
    BerlinHotspots,
}

impl MatchingAlgorithmType {
    fn create_matching_algorithm(self) -> MatchingAlgorithmResource {
        match self {
            MatchingAlgorithmType::Simple => create_simple_matching(),
            MatchingAlgorithmType::CostBased => create_cost_based_matching(DEFAULT_ETA_WEIGHT),
            MatchingAlgorithmType::Hungarian => create_hungarian_matching(DEFAULT_ETA_WEIGHT),
        }
    }
}

impl SimUiApp {
    pub fn new() -> Self {
        let defaults = AppDefaults::new();
        let start_epoch_ms = datetime_to_unix_ms(
            defaults.start_year,
            defaults.start_month,
            defaults.start_day,
            defaults.start_hour,
            defaults.start_minute,
        );

        let mut params = ScenarioParams {
            num_riders: defaults.num_riders,
            num_drivers: defaults.num_drivers,
            ..Default::default()
        }
        .with_request_window_hours(defaults.request_window_hours)
        .with_match_radius(km_to_cells(defaults.match_radius_km))
        .with_trip_duration_cells(
            km_to_cells(defaults.min_trip_km),
            km_to_cells(defaults.max_trip_km),
        )
        .with_epoch_ms(start_epoch_ms)
        .with_pricing_config(PricingConfig {
            base_fare: defaults.base_fare,
            per_km_rate: defaults.per_km_rate,
            commission_rate: defaults.commission_rate,
            surge_enabled: defaults.surge_enabled,
            surge_radius_k: defaults.surge_radius_k,
            surge_max_multiplier: defaults.surge_max_multiplier,
        })
        .with_rider_quote_config(RiderQuoteConfig {
            max_quote_rejections: defaults.max_quote_rejections,
            re_quote_delay_secs: 10,
            accept_probability: defaults.accept_probability,
            seed: if defaults.seed_enabled {
                defaults.seed_value
            } else {
                0_u64
            }
            .wrapping_add(0x0071_1073_beef_u64),
            max_willingness_to_pay: defaults.max_willingness_to_pay,
            max_acceptable_eta_ms: defaults.max_acceptable_eta_min.saturating_mul(60_000),
        })
        .with_driver_decision_config(DriverDecisionConfig {
            seed: if defaults.seed_enabled {
                defaults.seed_value
            } else {
                0_u64
            }
            .wrapping_add(0xdead_beef_u64),
            base_acceptance_score: defaults.driver_base_acceptance_score,
            fare_weight: defaults.driver_fare_weight,
            pickup_distance_penalty: defaults.driver_pickup_distance_penalty,
            ..Default::default()
        });
        if defaults.seed_enabled {
            params = params.with_seed(defaults.seed_value);
        }

        let mut world = World::new();
        build_scenario(&mut world, params);
        world.insert_resource(defaults.matching_algorithm.create_matching_algorithm());
        apply_batch_config(
            &mut world,
            defaults.batch_matching_enabled,
            defaults.batch_interval_secs,
        );
        sim_core::runner::initialize_simulation(&mut world);

        Self {
            world,
            schedule: simulation_schedule(),
            steps_executed: 0,
            auto_run: false,
            started: false,
            snapshot_interval_ms: 1000,
            speed_multiplier: 1000.0,
            sim_budget_ms: 0.0,
            last_frame_instant: None,
            num_riders: defaults.num_riders,
            num_drivers: defaults.num_drivers,
            initial_rider_count: defaults.initial_rider_count,
            initial_driver_count: defaults.initial_driver_count,
            request_window_hours: defaults.request_window_hours,
            driver_spread_hours: defaults.driver_spread_hours,
            simulation_duration_hours: defaults.simulation_duration_hours,
            match_radius_km: defaults.match_radius_km,
            min_trip_km: defaults.min_trip_km,
            max_trip_km: defaults.max_trip_km,
            seed_enabled: defaults.seed_enabled,
            seed_value: defaults.seed_value,
            grid_enabled: false,
            map_size_km: defaults.map_size_km,
            rider_cancel_min_mins: defaults.rider_cancel_min_mins,
            rider_cancel_max_mins: defaults.rider_cancel_max_mins,
            show_riders: true,
            show_drivers: true,
            show_driver_stats: true,
            hide_off_duty_drivers: true,
            matching_algorithm: defaults.matching_algorithm,
            matching_algorithm_changed: false,
            batch_matching_enabled: defaults.batch_matching_enabled,
            batch_interval_secs: defaults.batch_interval_secs,
            reposition_control_interval_secs: defaults.reposition_control_interval_secs,
            hotspot_weight: defaults.hotspot_weight,
            minimum_zone_reserve: defaults.minimum_zone_reserve,
            reposition_cooldown_secs: defaults.reposition_cooldown_secs,
            max_reposition_distance_km: defaults.max_reposition_distance_km,
            max_drivers_moved_per_cycle: defaults.max_drivers_moved_per_cycle,
            start_year: defaults.start_year,
            start_month: defaults.start_month,
            start_day: defaults.start_day,
            start_hour: defaults.start_hour,
            start_minute: defaults.start_minute,
            base_fare: defaults.base_fare,
            per_km_rate: defaults.per_km_rate,
            commission_rate: defaults.commission_rate,
            surge_enabled: defaults.surge_enabled,
            surge_radius_k: defaults.surge_radius_k,
            surge_max_multiplier: defaults.surge_max_multiplier,
            max_willingness_to_pay: defaults.max_willingness_to_pay,
            max_acceptable_eta_min: defaults.max_acceptable_eta_min,
            accept_probability: defaults.accept_probability,
            max_quote_rejections: defaults.max_quote_rejections,
            driver_base_acceptance_score: defaults.driver_base_acceptance_score,
            driver_fare_weight: defaults.driver_fare_weight,
            driver_pickup_distance_penalty: defaults.driver_pickup_distance_penalty,
            routing_mode: defaults.routing_mode,
            osrm_endpoint: defaults.osrm_endpoint,
            traffic_profile_mode: defaults.traffic_profile_mode,
            congestion_zones_enabled: defaults.congestion_zones_enabled,
            dynamic_congestion_enabled: defaults.dynamic_congestion_enabled,
            base_speed_enabled: defaults.base_speed_enabled,
            base_speed_kmh: defaults.base_speed_kmh,
            spawn_mode: defaults.spawn_mode,
            map_tiles: defaults.map_tiles,
        }
    }

    pub fn reset(&mut self) {
        self.rebuild_simulation(false, false);
    }

    pub fn start_simulation(&mut self) {
        self.rebuild_simulation(true, true);
        self.last_frame_instant = Some(Instant::now());
    }

    pub fn create_matching_algorithm(&self) -> MatchingAlgorithmResource {
        self.matching_algorithm.create_matching_algorithm()
    }

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

        params.reposition_policy_config = Some(RepositionPolicyConfig {
            control_interval_secs: self.reposition_control_interval_secs,
            hotspot_weight: self.hotspot_weight,
            minimum_zone_reserve: self.minimum_zone_reserve,
            cooldown_secs: self.reposition_cooldown_secs,
            max_reposition_distance_km: self.max_reposition_distance_km,
            max_drivers_moved_per_cycle: self.max_drivers_moved_per_cycle,
        });

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
                    0_u64
                }
                .wrapping_add(0x0071_1073_beef_u64),
                max_willingness_to_pay: self.max_willingness_to_pay,
                max_acceptable_eta_ms: self.max_acceptable_eta_min.saturating_mul(60_000),
            })
            .with_driver_decision_config(DriverDecisionConfig {
                seed: if self.seed_enabled {
                    self.seed_value
                } else {
                    0_u64
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
        params.base_speed_kmh = self.base_speed_enabled.then_some(self.base_speed_kmh);
        params.spawn_weighting = match self.spawn_mode {
            SpawnMode::Uniform => SpawnWeightingKind::Uniform,
            SpawnMode::BerlinHotspots => SpawnWeightingKind::BerlinHotspots,
        };
        params
    }

    pub fn run_steps(&mut self, steps: usize) {
        for _ in 0..steps {
            if !run_next_event(&mut self.world, &mut self.schedule) {
                break;
            }
            self.steps_executed += 1;
        }
    }

    pub fn run_until_done(&mut self) {
        loop {
            if !run_next_event(&mut self.world, &mut self.schedule) {
                break;
            }
            self.steps_executed += 1;
        }
    }

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

    fn rebuild_simulation(&mut self, started: bool, auto_run: bool) {
        let mut world = World::new();
        build_scenario(&mut world, self.current_params());
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
        apply_snapshot_interval(&mut world, self.snapshot_interval_ms);
        sim_core::runner::initialize_simulation(&mut world);

        self.world = world;
        self.schedule = simulation_schedule();
        self.steps_executed = 0;
        self.started = started;
        self.auto_run = auto_run;
        self.sim_budget_ms = 0.0;
        self.last_frame_instant = None;
        self.matching_algorithm_changed = false;
    }
}
