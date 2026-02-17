use bevy_ecs::prelude::World;
use std::path::PathBuf;
use std::time::Instant;

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

use crate::app::defaults::AppDefaults;
use crate::app::map_tiles::MapTileState;
use crate::app::presets::{
    delete_named_preset, export_library, import_library, list_named_presets, load_active_preset,
    load_named_preset, presets_file_path, save_autosave_preset, save_named_preset,
    DeleteNamedPresetOutcome, PresetMetadata, SaveNamedPresetOutcome, ScenarioPresetV1,
};
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
    pub preset_name_input: String,
    pub selected_preset_name: Option<String>,
    pub pending_overwrite_name: Option<String>,
    pub preset_status_message: Option<String>,
    pub active_preset_name: Option<String>,
    pub preset_names: Vec<String>,
    pub preset_load_error: Option<String>,
    pub preset_save_error: Option<String>,
    pub preset_transfer_path_input: String,
    preset_file_path: Option<PathBuf>,
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
        let mut defaults = AppDefaults::new();
        let mut preset_load_error = None;
        let mut active_preset_name = None;
        let mut preset_names = Vec::new();
        let mut selected_preset_name = None;
        let preset_file_path = match presets_file_path() {
            Ok(path) => Some(path),
            Err(error) => {
                preset_load_error = Some(format!("Preset storage disabled: {error}"));
                None
            }
        };
        if let Some(path) = preset_file_path.as_ref() {
            match load_active_preset(path) {
                Ok(Some(preset)) => preset.apply_to_defaults(&mut defaults),
                Ok(None) => {}
                Err(error) => {
                    preset_load_error = Some(format!("Preset load warning: {error}"));
                }
            }
            if preset_load_error.is_none() {
                match list_named_presets(path) {
                    Ok(metadata) => {
                        active_preset_name = metadata
                            .iter()
                            .find(|preset| preset.is_active)
                            .map(|preset| preset.name.clone());
                        preset_names = metadata.iter().map(|preset| preset.name.clone()).collect();
                        selected_preset_name = active_preset_name.clone();
                    }
                    Err(error) => {
                        preset_load_error = Some(format!("Preset load warning: {error}"));
                    }
                }
            }
        }
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
            preset_name_input: String::new(),
            selected_preset_name,
            pending_overwrite_name: None,
            preset_status_message: None,
            active_preset_name,
            preset_names,
            preset_load_error,
            preset_save_error: None,
            preset_transfer_path_input: String::new(),
            preset_file_path,
        }
    }

    pub fn reset(&mut self) {
        self.persist_autosave_preset();
        self.rebuild_simulation(false, false);
    }

    pub fn start_simulation(&mut self) {
        self.persist_autosave_preset();
        self.rebuild_simulation(true, true);
        self.last_frame_instant = Some(Instant::now());
    }

    pub fn persist_autosave_preset(&mut self) {
        if !self.can_mutate_presets() {
            self.preset_save_error = Some(
                "Preset mutating actions are disabled while simulation is running.".to_string(),
            );
            return;
        }

        let Some(path) = self.preset_file_path.as_ref() else {
            return;
        };
        let scenario = ScenarioPresetV1::from_app(self);
        match save_autosave_preset(path, &scenario) {
            Ok(()) => {
                self.preset_save_error = None;
            }
            Err(error) => {
                self.preset_save_error = Some(format!("Preset save warning: {error}"));
            }
        }
    }

    pub fn can_mutate_presets(&self) -> bool {
        !self.started
    }

    pub fn save_named_preset(&mut self) {
        if !self.can_mutate_presets() {
            self.preset_status_message =
                Some("Preset actions are disabled while simulation is running.".to_string());
            return;
        }

        let Some(path) = self.preset_file_path.clone() else {
            self.preset_status_message = Some("Preset storage is unavailable.".to_string());
            return;
        };

        let trimmed_name = self.preset_name_input.trim().to_string();
        if trimmed_name.is_empty() {
            self.preset_status_message = Some("Preset name cannot be empty.".to_string());
            return;
        }

        let scenario = ScenarioPresetV1::from_app(self);
        match save_named_preset(&path, &trimmed_name, &scenario, false) {
            Ok(SaveNamedPresetOutcome::Saved) => {
                self.pending_overwrite_name = None;
                self.preset_name_input = trimmed_name.clone();
                self.preset_status_message = Some(format!("Saved preset '{trimmed_name}'."));
                self.refresh_presets_from_store();
                self.selected_preset_name = Some(trimmed_name.clone());
            }
            Ok(SaveNamedPresetOutcome::Conflict) => {
                self.pending_overwrite_name = Some(trimmed_name.clone());
                self.preset_status_message = Some(format!(
                    "Preset '{trimmed_name}' already exists. Confirm overwrite to replace it."
                ));
            }
            Ok(SaveNamedPresetOutcome::Overwritten) => {
                self.pending_overwrite_name = None;
                self.preset_status_message = Some(format!("Overwrote preset '{trimmed_name}'."));
                self.refresh_presets_from_store();
                self.selected_preset_name = Some(trimmed_name.clone());
            }
            Err(error) => {
                self.pending_overwrite_name = None;
                self.preset_status_message = Some(format!("Preset save warning: {error}"));
            }
        }
    }

    pub fn confirm_overwrite_named_preset(&mut self) {
        if !self.can_mutate_presets() {
            self.preset_status_message =
                Some("Preset actions are disabled while simulation is running.".to_string());
            return;
        }

        let Some(name) = self.pending_overwrite_name.clone() else {
            return;
        };
        let Some(path) = self.preset_file_path.clone() else {
            self.preset_status_message = Some("Preset storage is unavailable.".to_string());
            return;
        };

        let scenario = ScenarioPresetV1::from_app(self);
        match save_named_preset(&path, &name, &scenario, true) {
            Ok(SaveNamedPresetOutcome::Overwritten | SaveNamedPresetOutcome::Saved) => {
                self.pending_overwrite_name = None;
                self.preset_name_input = name.clone();
                self.preset_status_message = Some(format!("Overwrote preset '{name}'."));
                self.refresh_presets_from_store();
                self.selected_preset_name = Some(name);
            }
            Ok(SaveNamedPresetOutcome::Conflict) => {
                self.pending_overwrite_name = Some(name.clone());
                self.preset_status_message =
                    Some(format!("Preset '{name}' still conflicts. Try again."));
            }
            Err(error) => {
                self.pending_overwrite_name = None;
                self.preset_status_message = Some(format!("Preset save warning: {error}"));
            }
        }
    }

    pub fn cancel_overwrite_named_preset(&mut self) {
        self.pending_overwrite_name = None;
        self.preset_status_message = Some("Overwrite canceled.".to_string());
    }

    pub fn load_selected_preset(&mut self) {
        if !self.can_mutate_presets() {
            self.preset_status_message =
                Some("Preset actions are disabled while simulation is running.".to_string());
            return;
        }

        let Some(name) = self.selected_preset_name.clone() else {
            self.preset_status_message = Some("Select a preset to load.".to_string());
            return;
        };
        let Some(path) = self.preset_file_path.clone() else {
            self.preset_status_message = Some("Preset storage is unavailable.".to_string());
            return;
        };

        match load_named_preset(&path, &name) {
            Ok(Some(preset)) => {
                self.apply_loaded_preset_to_controls(preset);
                self.pending_overwrite_name = None;
                self.preset_name_input = name.clone();
                self.preset_status_message = Some(format!("Loaded preset '{name}'."));
                self.refresh_presets_from_store();
                self.selected_preset_name = Some(name);
            }
            Ok(None) => {
                self.preset_status_message = Some(format!("Preset '{name}' no longer exists."));
                self.refresh_presets_from_store();
            }
            Err(error) => {
                self.preset_status_message = Some(format!("Preset load warning: {error}"));
            }
        }
    }

    pub fn delete_selected_preset(&mut self) {
        if !self.can_mutate_presets() {
            self.preset_status_message =
                Some("Preset actions are disabled while simulation is running.".to_string());
            return;
        }

        let Some(name) = self.selected_preset_name.clone() else {
            self.preset_status_message = Some("Select a preset to delete.".to_string());
            return;
        };
        let Some(path) = self.preset_file_path.clone() else {
            self.preset_status_message = Some("Preset storage is unavailable.".to_string());
            return;
        };

        match delete_named_preset(&path, &name) {
            Ok(DeleteNamedPresetOutcome::Deleted) => {
                if self.pending_overwrite_name.as_deref() == Some(name.as_str()) {
                    self.pending_overwrite_name = None;
                }
                self.preset_status_message = Some(format!("Deleted preset '{name}'."));
                self.refresh_presets_from_store();
                self.selected_preset_name = self
                    .selected_preset_name
                    .as_ref()
                    .and_then(|selected| {
                        self.preset_names
                            .iter()
                            .any(|candidate| candidate == selected)
                            .then(|| selected.clone())
                    })
                    .or_else(|| self.active_preset_name.clone())
                    .or_else(|| self.preset_names.first().cloned());
            }
            Ok(DeleteNamedPresetOutcome::NotFound) => {
                self.preset_status_message = Some(format!("Preset '{name}' no longer exists."));
                self.refresh_presets_from_store();
            }
            Err(error) => {
                self.preset_status_message = Some(format!("Preset delete warning: {error}"));
            }
        }
    }

    pub fn export_preset_library(&mut self) {
        if !self.can_mutate_presets() {
            self.preset_status_message =
                Some("Preset actions are disabled while simulation is running.".to_string());
            return;
        }

        let Some(store_path) = self.preset_file_path.clone() else {
            self.preset_status_message = Some("Preset storage is unavailable.".to_string());
            return;
        };

        let Some(transfer_path) = self.parse_transfer_path_input() else {
            return;
        };

        match export_library(&store_path, &transfer_path) {
            Ok(()) => {
                self.preset_status_message = Some(format!(
                    "Exported preset library to '{}'.",
                    transfer_path.display()
                ));
            }
            Err(error) => {
                self.preset_status_message = Some(format!("Preset export warning: {error}"));
            }
        }
    }

    pub fn import_preset_library(&mut self) {
        if !self.can_mutate_presets() {
            self.preset_status_message =
                Some("Preset actions are disabled while simulation is running.".to_string());
            return;
        }

        let Some(store_path) = self.preset_file_path.clone() else {
            self.preset_status_message = Some("Preset storage is unavailable.".to_string());
            return;
        };

        let Some(transfer_path) = self.parse_transfer_path_input() else {
            return;
        };

        let previous_selection = self.selected_preset_name.clone();
        match import_library(&store_path, &transfer_path) {
            Ok(()) => {
                self.pending_overwrite_name = None;
                self.refresh_presets_from_store();
                self.reconcile_selected_preset(previous_selection);
                self.preset_status_message = Some(format!(
                    "Imported preset library from '{}'.",
                    transfer_path.display()
                ));
            }
            Err(error) => {
                self.preset_status_message = Some(format!("Preset import warning: {error}"));
            }
        }
    }

    fn parse_transfer_path_input(&mut self) -> Option<PathBuf> {
        let trimmed = self.preset_transfer_path_input.trim();
        if trimmed.is_empty() {
            self.preset_status_message =
                Some("Transfer path cannot be empty for library import/export.".to_string());
            return None;
        }
        Some(PathBuf::from(trimmed))
    }

    fn refresh_presets_from_store(&mut self) {
        let Some(path) = self.preset_file_path.clone() else {
            self.preset_names.clear();
            self.active_preset_name = None;
            return;
        };

        match list_named_presets(&path) {
            Ok(metadata) => {
                self.sync_preset_lists(metadata);
                self.preset_load_error = None;
            }
            Err(error) => {
                self.preset_load_error = Some(format!("Preset load warning: {error}"));
            }
        }
    }

    fn sync_preset_lists(&mut self, metadata: Vec<PresetMetadata>) {
        self.active_preset_name = metadata
            .iter()
            .find(|preset| preset.is_active)
            .map(|preset| preset.name.clone());
        self.preset_names = metadata.into_iter().map(|preset| preset.name).collect();
    }

    fn reconcile_selected_preset(&mut self, previous_selection: Option<String>) {
        self.selected_preset_name = previous_selection
            .filter(|selected| {
                self.preset_names
                    .iter()
                    .any(|candidate| candidate == selected)
            })
            .or_else(|| self.active_preset_name.clone())
            .or_else(|| self.preset_names.first().cloned());
    }

    fn apply_loaded_preset_to_controls(&mut self, preset: ScenarioPresetV1) {
        let mut defaults = AppDefaults::new();
        preset.apply_to_defaults(&mut defaults);

        self.num_riders = defaults.num_riders;
        self.num_drivers = defaults.num_drivers;
        self.initial_rider_count = defaults.initial_rider_count;
        self.initial_driver_count = defaults.initial_driver_count;
        self.request_window_hours = defaults.request_window_hours;
        self.driver_spread_hours = defaults.driver_spread_hours;
        self.simulation_duration_hours = defaults.simulation_duration_hours;
        self.match_radius_km = defaults.match_radius_km;
        self.min_trip_km = defaults.min_trip_km;
        self.max_trip_km = defaults.max_trip_km;
        self.seed_enabled = defaults.seed_enabled;
        self.seed_value = defaults.seed_value;
        self.map_size_km = defaults.map_size_km;
        self.rider_cancel_min_mins = defaults.rider_cancel_min_mins;
        self.rider_cancel_max_mins = defaults.rider_cancel_max_mins;
        self.matching_algorithm = defaults.matching_algorithm;
        self.batch_matching_enabled = defaults.batch_matching_enabled;
        self.batch_interval_secs = defaults.batch_interval_secs;
        self.start_year = defaults.start_year;
        self.start_month = defaults.start_month;
        self.start_day = defaults.start_day;
        self.start_hour = defaults.start_hour;
        self.start_minute = defaults.start_minute;
        self.base_fare = defaults.base_fare;
        self.per_km_rate = defaults.per_km_rate;
        self.commission_rate = defaults.commission_rate;
        self.surge_enabled = defaults.surge_enabled;
        self.surge_radius_k = defaults.surge_radius_k;
        self.surge_max_multiplier = defaults.surge_max_multiplier;
        self.max_willingness_to_pay = defaults.max_willingness_to_pay;
        self.max_acceptable_eta_min = defaults.max_acceptable_eta_min;
        self.accept_probability = defaults.accept_probability;
        self.max_quote_rejections = defaults.max_quote_rejections;
        self.driver_base_acceptance_score = defaults.driver_base_acceptance_score;
        self.driver_fare_weight = defaults.driver_fare_weight;
        self.driver_pickup_distance_penalty = defaults.driver_pickup_distance_penalty;
        self.routing_mode = defaults.routing_mode;
        self.osrm_endpoint = defaults.osrm_endpoint;
        self.traffic_profile_mode = defaults.traffic_profile_mode;
        self.congestion_zones_enabled = defaults.congestion_zones_enabled;
        self.dynamic_congestion_enabled = defaults.dynamic_congestion_enabled;
        self.base_speed_enabled = defaults.base_speed_enabled;
        self.base_speed_kmh = defaults.base_speed_kmh;
        self.spawn_mode = defaults.spawn_mode;
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

impl Drop for SimUiApp {
    fn drop(&mut self) {
        self.persist_autosave_preset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_test_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("sim_ui_runtime_guard_{label}_{nanos}.json"))
    }

    #[test]
    fn autosave_is_blocked_while_simulation_is_running_and_reenabled_after_stop() {
        let path = unique_test_path("persist_guard");
        let mut app = SimUiApp::new();
        app.preset_file_path = Some(path.clone());

        app.num_riders = 111;
        app.started = false;
        let expected_first = ScenarioPresetV1::from_app(&app);
        app.persist_autosave_preset();

        let first_saved = load_active_preset(&path)
            .expect("initial autosave should succeed")
            .expect("active autosave preset should exist");
        assert_eq!(first_saved, expected_first);
        assert!(app.preset_save_error.is_none());

        app.num_riders = 222;
        app.started = true;
        app.persist_autosave_preset();

        let blocked_saved = load_active_preset(&path)
            .expect("blocked save should keep previous data readable")
            .expect("active autosave preset should still exist");
        assert_eq!(blocked_saved, expected_first);
        assert_eq!(
            app.preset_save_error.as_deref(),
            Some("Preset mutating actions are disabled while simulation is running.")
        );

        app.started = false;
        let expected_resumed = ScenarioPresetV1::from_app(&app);
        app.persist_autosave_preset();

        let resumed_saved = load_active_preset(&path)
            .expect("save should resume after simulation stops")
            .expect("active autosave preset should exist after resume");
        assert_eq!(resumed_saved, expected_resumed);
        assert!(app.preset_save_error.is_none());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn can_mutate_presets_tracks_started_state() {
        let mut app = SimUiApp::new();
        app.started = false;
        assert!(app.can_mutate_presets());

        app.started = true;
        assert!(!app.can_mutate_presets());
    }

    #[test]
    fn save_named_preset_rejects_empty_name() {
        let path = unique_test_path("empty_name");
        let mut app = SimUiApp::new();
        app.preset_file_path = Some(path.clone());
        app.preset_name_input = "   ".to_string();

        app.save_named_preset();

        assert_eq!(
            app.preset_status_message.as_deref(),
            Some("Preset name cannot be empty.")
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn duplicate_save_requires_explicit_overwrite_confirmation() {
        let path = unique_test_path("overwrite_flow");
        let mut app = SimUiApp::new();
        app.preset_file_path = Some(path.clone());

        app.num_riders = 100;
        app.preset_name_input = "weekday".to_string();
        app.save_named_preset();
        assert!(app.pending_overwrite_name.is_none());

        app.num_riders = 250;
        app.preset_name_input = "weekday".to_string();
        app.save_named_preset();
        assert_eq!(app.pending_overwrite_name.as_deref(), Some("weekday"));

        app.confirm_overwrite_named_preset();
        assert!(app.pending_overwrite_name.is_none());

        app.selected_preset_name = Some("weekday".to_string());
        app.num_riders = 1;
        app.load_selected_preset();
        assert_eq!(app.num_riders, 250);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn save_and_delete_are_blocked_while_running() {
        let path = unique_test_path("guarded_actions");
        let mut app = SimUiApp::new();
        app.preset_file_path = Some(path.clone());

        app.started = false;
        app.preset_name_input = "guarded".to_string();
        app.save_named_preset();

        app.started = true;
        app.preset_name_input = "guarded".to_string();
        app.save_named_preset();
        assert_eq!(
            app.preset_status_message.as_deref(),
            Some("Preset actions are disabled while simulation is running.")
        );

        app.selected_preset_name = Some("guarded".to_string());
        app.delete_selected_preset();
        assert_eq!(
            app.preset_status_message.as_deref(),
            Some("Preset actions are disabled while simulation is running.")
        );

        app.started = false;
        app.selected_preset_name = Some("guarded".to_string());
        app.load_selected_preset();
        assert_eq!(app.selected_preset_name.as_deref(), Some("guarded"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn loading_preset_updates_controls_without_starting_simulation() {
        let path = unique_test_path("load_no_start");
        let mut app = SimUiApp::new();
        app.preset_file_path = Some(path.clone());

        app.started = false;
        app.auto_run = false;
        app.steps_executed = 77;
        app.num_riders = 321;
        app.preset_name_input = "load_test".to_string();
        app.save_named_preset();

        app.num_riders = 7;
        app.selected_preset_name = Some("load_test".to_string());
        app.load_selected_preset();

        assert_eq!(app.num_riders, 321);
        assert!(!app.started);
        assert!(!app.auto_run);
        assert_eq!(app.steps_executed, 77);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn deleting_selected_active_preset_updates_active_indicator() {
        let path = unique_test_path("delete_active");
        let mut app = SimUiApp::new();
        app.preset_file_path = Some(path.clone());

        app.preset_name_input = "alpha".to_string();
        app.save_named_preset();
        app.selected_preset_name = Some("alpha".to_string());
        app.load_selected_preset();
        assert_eq!(app.active_preset_name.as_deref(), Some("alpha"));

        app.delete_selected_preset();
        assert!(app
            .preset_names
            .iter()
            .all(|preset_name| preset_name != "alpha"));
        assert!(app.active_preset_name.is_none());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_library_writes_transfer_file() {
        let store_path = unique_test_path("export_store");
        let transfer_path = unique_test_path("export_transfer");
        let mut app = SimUiApp::new();
        app.preset_file_path = Some(store_path.clone());

        app.preset_name_input = "alpha".to_string();
        app.save_named_preset();
        app.preset_transfer_path_input = transfer_path.display().to_string();

        app.export_preset_library();

        let transfer_contents =
            fs::read_to_string(&transfer_path).expect("transfer file should be written");
        assert!(transfer_contents.contains("\"presets\""));
        assert!(transfer_contents.contains("\"alpha\""));
        let expected = format!("Exported preset library to '{}'.", transfer_path.display());
        assert_eq!(
            app.preset_status_message.as_deref(),
            Some(expected.as_str())
        );

        let _ = fs::remove_file(store_path);
        let _ = fs::remove_file(transfer_path);
    }

    #[test]
    fn import_library_success_refreshes_names_and_active_selection() {
        let source_store_path = unique_test_path("import_source_store");
        let target_store_path = unique_test_path("import_target_store");
        let transfer_path = unique_test_path("import_transfer");

        let mut source_app = SimUiApp::new();
        source_app.preset_file_path = Some(source_store_path.clone());
        source_app.preset_name_input = "baseline".to_string();
        source_app.save_named_preset();
        source_app.preset_name_input = "imported".to_string();
        source_app.save_named_preset();
        source_app.selected_preset_name = Some("imported".to_string());
        source_app.load_selected_preset();
        source_app.preset_transfer_path_input = transfer_path.display().to_string();
        source_app.export_preset_library();

        let mut target_app = SimUiApp::new();
        target_app.preset_file_path = Some(target_store_path.clone());
        target_app.preset_name_input = "stale".to_string();
        target_app.save_named_preset();
        target_app.selected_preset_name = Some("stale".to_string());
        target_app.load_selected_preset();

        target_app.preset_transfer_path_input = transfer_path.display().to_string();
        target_app.import_preset_library();

        assert!(target_app
            .preset_names
            .iter()
            .any(|preset_name| preset_name == "baseline"));
        assert!(target_app
            .preset_names
            .iter()
            .any(|preset_name| preset_name == "imported"));
        assert!(target_app
            .preset_names
            .iter()
            .all(|preset_name| preset_name != "stale"));
        assert_eq!(target_app.active_preset_name.as_deref(), Some("imported"));
        assert_eq!(target_app.selected_preset_name.as_deref(), Some("imported"));
        let expected = format!(
            "Imported preset library from '{}'.",
            transfer_path.display()
        );
        assert_eq!(
            target_app.preset_status_message.as_deref(),
            Some(expected.as_str())
        );

        let _ = fs::remove_file(source_store_path);
        let _ = fs::remove_file(target_store_path);
        let _ = fs::remove_file(transfer_path);
    }

    #[test]
    fn invalid_import_reports_warning_without_mutating_current_presets() {
        let store_path = unique_test_path("invalid_import_store");
        let transfer_path = unique_test_path("invalid_import_transfer");
        let mut app = SimUiApp::new();
        app.preset_file_path = Some(store_path.clone());

        app.preset_name_input = "kept".to_string();
        app.save_named_preset();
        app.selected_preset_name = Some("kept".to_string());
        app.load_selected_preset();

        fs::write(&transfer_path, "{}").expect("invalid transfer payload should be written");
        app.preset_transfer_path_input = transfer_path.display().to_string();

        let preset_names_before = app.preset_names.clone();
        let active_before = app.active_preset_name.clone();
        let selected_before = app.selected_preset_name.clone();

        app.import_preset_library();

        assert!(app
            .preset_status_message
            .as_deref()
            .unwrap_or_default()
            .starts_with("Preset import warning:"));
        assert_eq!(app.preset_names, preset_names_before);
        assert_eq!(app.active_preset_name, active_before);
        assert_eq!(app.selected_preset_name, selected_before);

        let metadata = list_named_presets(&store_path).expect("metadata should still be readable");
        assert!(metadata.iter().any(|entry| entry.name == "kept"));

        let _ = fs::remove_file(store_path);
        let _ = fs::remove_file(transfer_path);
    }

    #[test]
    fn import_library_is_blocked_while_simulation_is_running() {
        let store_path = unique_test_path("running_import_store");
        let transfer_path = unique_test_path("running_import_transfer");
        let mut app = SimUiApp::new();
        app.preset_file_path = Some(store_path.clone());
        app.started = true;
        app.preset_transfer_path_input = transfer_path.display().to_string();

        app.import_preset_library();

        assert_eq!(
            app.preset_status_message.as_deref(),
            Some("Preset actions are disabled while simulation is running.")
        );

        let _ = fs::remove_file(store_path);
        let _ = fs::remove_file(transfer_path);
    }
}
