use serde::{Deserialize, Serialize};

use crate::app::defaults::AppDefaults;
use crate::app::simulation::{
    MatchingAlgorithmType, RoutingMode, SimUiApp, SpawnMode, TrafficProfileMode,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(super) enum MatchingAlgorithmPresetV1 {
    Simple,
    CostBased,
    Hungarian,
}

impl From<MatchingAlgorithmType> for MatchingAlgorithmPresetV1 {
    fn from(value: MatchingAlgorithmType) -> Self {
        match value {
            MatchingAlgorithmType::Simple => Self::Simple,
            MatchingAlgorithmType::CostBased => Self::CostBased,
            MatchingAlgorithmType::Hungarian => Self::Hungarian,
        }
    }
}

impl From<MatchingAlgorithmPresetV1> for MatchingAlgorithmType {
    fn from(value: MatchingAlgorithmPresetV1) -> Self {
        match value {
            MatchingAlgorithmPresetV1::Simple => Self::Simple,
            MatchingAlgorithmPresetV1::CostBased => Self::CostBased,
            MatchingAlgorithmPresetV1::Hungarian => Self::Hungarian,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(super) enum RoutingModePresetV1 {
    H3Grid,
    Osrm,
}

impl From<RoutingMode> for RoutingModePresetV1 {
    fn from(value: RoutingMode) -> Self {
        match value {
            RoutingMode::H3Grid => Self::H3Grid,
            RoutingMode::Osrm => Self::Osrm,
        }
    }
}

impl From<RoutingModePresetV1> for RoutingMode {
    fn from(value: RoutingModePresetV1) -> Self {
        match value {
            RoutingModePresetV1::H3Grid => Self::H3Grid,
            RoutingModePresetV1::Osrm => Self::Osrm,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(super) enum TrafficProfileModePresetV1 {
    None,
    Berlin,
}

impl From<TrafficProfileMode> for TrafficProfileModePresetV1 {
    fn from(value: TrafficProfileMode) -> Self {
        match value {
            TrafficProfileMode::None => Self::None,
            TrafficProfileMode::Berlin => Self::Berlin,
        }
    }
}

impl From<TrafficProfileModePresetV1> for TrafficProfileMode {
    fn from(value: TrafficProfileModePresetV1) -> Self {
        match value {
            TrafficProfileModePresetV1::None => Self::None,
            TrafficProfileModePresetV1::Berlin => Self::Berlin,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(super) enum SpawnModePresetV1 {
    Uniform,
    BerlinHotspots,
}

impl From<SpawnMode> for SpawnModePresetV1 {
    fn from(value: SpawnMode) -> Self {
        match value {
            SpawnMode::Uniform => Self::Uniform,
            SpawnMode::BerlinHotspots => Self::BerlinHotspots,
        }
    }
}

impl From<SpawnModePresetV1> for SpawnMode {
    fn from(value: SpawnModePresetV1) -> Self {
        match value {
            SpawnModePresetV1::Uniform => Self::Uniform,
            SpawnModePresetV1::BerlinHotspots => Self::BerlinHotspots,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScenarioPresetV1 {
    pub(super) num_riders: usize,
    pub(super) num_drivers: usize,
    pub(super) initial_rider_count: usize,
    pub(super) initial_driver_count: usize,
    pub(super) request_window_hours: u64,
    pub(super) driver_spread_hours: u64,
    pub(super) simulation_duration_hours: u64,
    pub(super) match_radius_km: f64,
    pub(super) min_trip_km: f64,
    pub(super) max_trip_km: f64,
    pub(super) map_size_km: f64,
    pub(super) rider_cancel_min_mins: u64,
    pub(super) rider_cancel_max_mins: u64,
    pub(super) seed_enabled: bool,
    pub(super) seed_value: u64,
    pub(super) matching_algorithm: MatchingAlgorithmPresetV1,
    pub(super) batch_matching_enabled: bool,
    pub(super) batch_interval_secs: u64,
    pub(super) base_fare: f64,
    pub(super) per_km_rate: f64,
    pub(super) commission_rate: f64,
    pub(super) surge_enabled: bool,
    pub(super) surge_radius_k: u32,
    pub(super) surge_max_multiplier: f64,
    pub(super) max_willingness_to_pay: f64,
    pub(super) max_acceptable_eta_min: u64,
    pub(super) accept_probability: f64,
    pub(super) max_quote_rejections: u32,
    pub(super) driver_base_acceptance_score: f64,
    pub(super) driver_fare_weight: f64,
    pub(super) driver_pickup_distance_penalty: f64,
    pub(super) routing_mode: RoutingModePresetV1,
    pub(super) osrm_endpoint: String,
    pub(super) traffic_profile_mode: TrafficProfileModePresetV1,
    pub(super) congestion_zones_enabled: bool,
    pub(super) dynamic_congestion_enabled: bool,
    pub(super) base_speed_enabled: bool,
    pub(super) base_speed_kmh: f64,
    pub(super) spawn_mode: SpawnModePresetV1,
    pub(super) start_year: i32,
    pub(super) start_month: u32,
    pub(super) start_day: u32,
    pub(super) start_hour: u32,
    pub(super) start_minute: u32,
}

impl ScenarioPresetV1 {
    #[cfg(test)]
    pub(crate) fn from_defaults(defaults: &AppDefaults) -> Self {
        Self {
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
            map_size_km: defaults.map_size_km,
            rider_cancel_min_mins: defaults.rider_cancel_min_mins,
            rider_cancel_max_mins: defaults.rider_cancel_max_mins,
            seed_enabled: defaults.seed_enabled,
            seed_value: defaults.seed_value,
            matching_algorithm: defaults.matching_algorithm.into(),
            batch_matching_enabled: defaults.batch_matching_enabled,
            batch_interval_secs: defaults.batch_interval_secs,
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
            routing_mode: defaults.routing_mode.into(),
            osrm_endpoint: defaults.osrm_endpoint.clone(),
            traffic_profile_mode: defaults.traffic_profile_mode.into(),
            congestion_zones_enabled: defaults.congestion_zones_enabled,
            dynamic_congestion_enabled: defaults.dynamic_congestion_enabled,
            base_speed_enabled: defaults.base_speed_enabled,
            base_speed_kmh: defaults.base_speed_kmh,
            spawn_mode: defaults.spawn_mode.into(),
            start_year: defaults.start_year,
            start_month: defaults.start_month,
            start_day: defaults.start_day,
            start_hour: defaults.start_hour,
            start_minute: defaults.start_minute,
        }
    }

    pub(crate) fn from_app(app: &SimUiApp) -> Self {
        Self {
            num_riders: app.num_riders,
            num_drivers: app.num_drivers,
            initial_rider_count: app.initial_rider_count,
            initial_driver_count: app.initial_driver_count,
            request_window_hours: app.request_window_hours,
            driver_spread_hours: app.driver_spread_hours,
            simulation_duration_hours: app.simulation_duration_hours,
            match_radius_km: app.match_radius_km,
            min_trip_km: app.min_trip_km,
            max_trip_km: app.max_trip_km,
            map_size_km: app.map_size_km,
            rider_cancel_min_mins: app.rider_cancel_min_mins,
            rider_cancel_max_mins: app.rider_cancel_max_mins,
            seed_enabled: app.seed_enabled,
            seed_value: app.seed_value,
            matching_algorithm: app.matching_algorithm.into(),
            batch_matching_enabled: app.batch_matching_enabled,
            batch_interval_secs: app.batch_interval_secs,
            base_fare: app.base_fare,
            per_km_rate: app.per_km_rate,
            commission_rate: app.commission_rate,
            surge_enabled: app.surge_enabled,
            surge_radius_k: app.surge_radius_k,
            surge_max_multiplier: app.surge_max_multiplier,
            max_willingness_to_pay: app.max_willingness_to_pay,
            max_acceptable_eta_min: app.max_acceptable_eta_min,
            accept_probability: app.accept_probability,
            max_quote_rejections: app.max_quote_rejections,
            driver_base_acceptance_score: app.driver_base_acceptance_score,
            driver_fare_weight: app.driver_fare_weight,
            driver_pickup_distance_penalty: app.driver_pickup_distance_penalty,
            routing_mode: app.routing_mode.into(),
            osrm_endpoint: app.osrm_endpoint.clone(),
            traffic_profile_mode: app.traffic_profile_mode.into(),
            congestion_zones_enabled: app.congestion_zones_enabled,
            dynamic_congestion_enabled: app.dynamic_congestion_enabled,
            base_speed_enabled: app.base_speed_enabled,
            base_speed_kmh: app.base_speed_kmh,
            spawn_mode: app.spawn_mode.into(),
            start_year: app.start_year,
            start_month: app.start_month,
            start_day: app.start_day,
            start_hour: app.start_hour,
            start_minute: app.start_minute,
        }
    }

    pub(crate) fn apply_to_defaults(self, defaults: &mut AppDefaults) {
        let normalized = self.normalized(defaults);

        defaults.num_riders = normalized.num_riders;
        defaults.num_drivers = normalized.num_drivers;
        defaults.initial_rider_count = normalized.initial_rider_count;
        defaults.initial_driver_count = normalized.initial_driver_count;
        defaults.request_window_hours = normalized.request_window_hours;
        defaults.driver_spread_hours = normalized.driver_spread_hours;
        defaults.simulation_duration_hours = normalized.simulation_duration_hours;
        defaults.match_radius_km = normalized.match_radius_km;
        defaults.min_trip_km = normalized.min_trip_km;
        defaults.max_trip_km = normalized.max_trip_km;
        defaults.map_size_km = normalized.map_size_km;
        defaults.rider_cancel_min_mins = normalized.rider_cancel_min_mins;
        defaults.rider_cancel_max_mins = normalized.rider_cancel_max_mins;
        defaults.seed_enabled = normalized.seed_enabled;
        defaults.seed_value = normalized.seed_value;
        defaults.matching_algorithm = normalized.matching_algorithm.into();
        defaults.batch_matching_enabled = normalized.batch_matching_enabled;
        defaults.batch_interval_secs = normalized.batch_interval_secs;
        defaults.base_fare = normalized.base_fare;
        defaults.per_km_rate = normalized.per_km_rate;
        defaults.commission_rate = normalized.commission_rate;
        defaults.surge_enabled = normalized.surge_enabled;
        defaults.surge_radius_k = normalized.surge_radius_k;
        defaults.surge_max_multiplier = normalized.surge_max_multiplier;
        defaults.max_willingness_to_pay = normalized.max_willingness_to_pay;
        defaults.max_acceptable_eta_min = normalized.max_acceptable_eta_min;
        defaults.accept_probability = normalized.accept_probability;
        defaults.max_quote_rejections = normalized.max_quote_rejections;
        defaults.driver_base_acceptance_score = normalized.driver_base_acceptance_score;
        defaults.driver_fare_weight = normalized.driver_fare_weight;
        defaults.driver_pickup_distance_penalty = normalized.driver_pickup_distance_penalty;
        defaults.routing_mode = normalized.routing_mode.into();
        defaults.osrm_endpoint = normalized.osrm_endpoint;
        defaults.traffic_profile_mode = normalized.traffic_profile_mode.into();
        defaults.congestion_zones_enabled = normalized.congestion_zones_enabled;
        defaults.dynamic_congestion_enabled = normalized.dynamic_congestion_enabled;
        defaults.base_speed_enabled = normalized.base_speed_enabled;
        defaults.base_speed_kmh = normalized.base_speed_kmh;
        defaults.spawn_mode = normalized.spawn_mode.into();
        defaults.start_year = normalized.start_year;
        defaults.start_month = normalized.start_month;
        defaults.start_day = normalized.start_day;
        defaults.start_hour = normalized.start_hour;
        defaults.start_minute = normalized.start_minute;
    }

    fn normalized(mut self, defaults: &AppDefaults) -> Self {
        self.num_riders = self.num_riders.clamp(1, 10_000);
        self.num_drivers = self.num_drivers.clamp(1, 10_000);
        self.initial_rider_count = self.initial_rider_count.clamp(0, 10_000);
        self.initial_driver_count = self.initial_driver_count.clamp(0, 10_000);
        self.request_window_hours = self.request_window_hours.clamp(1, 24);
        self.driver_spread_hours = self.driver_spread_hours.clamp(1, 24);
        self.simulation_duration_hours = self.simulation_duration_hours.clamp(1, 168);
        self.match_radius_km = self.match_radius_km.clamp(0.0, 20.0);
        self.min_trip_km = self.min_trip_km.clamp(0.1, 100.0);
        self.max_trip_km = self.max_trip_km.clamp(0.1, 200.0).max(self.min_trip_km);
        self.map_size_km = self.map_size_km.clamp(1.0, 200.0);
        self.rider_cancel_min_mins = self.rider_cancel_min_mins.clamp(1, 600);
        self.rider_cancel_max_mins = self
            .rider_cancel_max_mins
            .clamp(1, 600)
            .max(self.rider_cancel_min_mins);
        self.batch_interval_secs = self.batch_interval_secs.clamp(1, 120);
        self.base_fare = self.base_fare.clamp(0.0, 100.0);
        self.per_km_rate = self.per_km_rate.clamp(0.0, 100.0);
        self.commission_rate = self.commission_rate.clamp(0.0, 1.0);
        self.surge_radius_k = self.surge_radius_k.clamp(1, 5);
        self.surge_max_multiplier = self.surge_max_multiplier.clamp(1.0, 5.0);
        self.max_willingness_to_pay = self.max_willingness_to_pay.clamp(1.0, 500.0);
        self.max_acceptable_eta_min = self.max_acceptable_eta_min.clamp(1, 60);
        self.accept_probability = self.accept_probability.clamp(0.0, 1.0);
        self.max_quote_rejections = self.max_quote_rejections.clamp(1, 10);
        self.driver_base_acceptance_score = self.driver_base_acceptance_score.clamp(-10.0, 10.0);
        self.driver_fare_weight = self.driver_fare_weight.clamp(0.0, 1.0);
        self.driver_pickup_distance_penalty = self.driver_pickup_distance_penalty.clamp(-10.0, 0.0);
        self.base_speed_kmh = self.base_speed_kmh.clamp(10.0, 200.0);
        self.start_year = self.start_year.clamp(1970, 2100);
        self.start_month = self.start_month.clamp(1, 12);
        self.start_day = self.start_day.clamp(1, 31);
        self.start_hour = self.start_hour.clamp(0, 23);
        self.start_minute = self.start_minute.clamp(0, 59);
        if self.osrm_endpoint.trim().is_empty() {
            self.osrm_endpoint = defaults.osrm_endpoint.clone();
        }
        self
    }
}
