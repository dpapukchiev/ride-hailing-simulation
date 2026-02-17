use std::collections::HashSet;
use std::fmt;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::app::defaults::AppDefaults;
use crate::app::simulation::{
    MatchingAlgorithmType, RoutingMode, SimUiApp, SpawnMode, TrafficProfileMode,
};

pub(crate) const PRESETS_FILE_NAME: &str = "sim_ui_presets.json";
const PRESET_FILE_VERSION: u32 = 1;
const AUTOSAVE_PRESET_NAME: &str = "autosave";

#[derive(Debug)]
pub(crate) enum PresetStoreError {
    Io(String),
    InvalidFormat(String),
}

impl fmt::Display for PresetStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PresetStoreError::Io(message) => write!(f, "{message}"),
            PresetStoreError::InvalidFormat(message) => write!(f, "{message}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PresetLibraryV1 {
    version: u32,
    active_preset: Option<String>,
    presets: Vec<NamedPresetV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PresetMetadata {
    pub(crate) name: String,
    pub(crate) is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SaveNamedPresetOutcome {
    Saved,
    Overwritten,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DeleteNamedPresetOutcome {
    Deleted,
    NotFound,
}

impl PresetLibraryV1 {
    fn empty() -> Self {
        Self {
            version: PRESET_FILE_VERSION,
            active_preset: None,
            presets: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NamedPresetV1 {
    name: String,
    scenario: ScenarioPresetV1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum MatchingAlgorithmPresetV1 {
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
enum RoutingModePresetV1 {
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
enum TrafficProfileModePresetV1 {
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
enum SpawnModePresetV1 {
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
    num_riders: usize,
    num_drivers: usize,
    initial_rider_count: usize,
    initial_driver_count: usize,
    request_window_hours: u64,
    driver_spread_hours: u64,
    simulation_duration_hours: u64,
    match_radius_km: f64,
    min_trip_km: f64,
    max_trip_km: f64,
    map_size_km: f64,
    rider_cancel_min_mins: u64,
    rider_cancel_max_mins: u64,
    seed_enabled: bool,
    seed_value: u64,
    matching_algorithm: MatchingAlgorithmPresetV1,
    batch_matching_enabled: bool,
    batch_interval_secs: u64,
    base_fare: f64,
    per_km_rate: f64,
    commission_rate: f64,
    surge_enabled: bool,
    surge_radius_k: u32,
    surge_max_multiplier: f64,
    max_willingness_to_pay: f64,
    max_acceptable_eta_min: u64,
    accept_probability: f64,
    max_quote_rejections: u32,
    driver_base_acceptance_score: f64,
    driver_fare_weight: f64,
    driver_pickup_distance_penalty: f64,
    routing_mode: RoutingModePresetV1,
    osrm_endpoint: String,
    traffic_profile_mode: TrafficProfileModePresetV1,
    congestion_zones_enabled: bool,
    dynamic_congestion_enabled: bool,
    base_speed_enabled: bool,
    base_speed_kmh: f64,
    spawn_mode: SpawnModePresetV1,
    start_year: i32,
    start_month: u32,
    start_day: u32,
    start_hour: u32,
    start_minute: u32,
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

pub(crate) fn presets_file_path() -> Result<PathBuf, PresetStoreError> {
    let cwd = std::env::current_dir().map_err(|error| {
        PresetStoreError::Io(format!("failed to read current directory: {error}"))
    })?;
    Ok(cwd.join(PRESETS_FILE_NAME))
}

pub(crate) fn load_active_preset(
    path: &Path,
) -> Result<Option<ScenarioPresetV1>, PresetStoreError> {
    let library = load_library(path)?;
    let Some(active_name) = library.active_preset.as_ref() else {
        return Ok(None);
    };
    Ok(library
        .presets
        .iter()
        .find(|preset| preset.name == *active_name)
        .map(|preset| preset.scenario.clone()))
}

pub(crate) fn save_autosave_preset(
    path: &Path,
    scenario: &ScenarioPresetV1,
) -> Result<(), PresetStoreError> {
    let mut library = match load_library(path) {
        Ok(library) => library,
        Err(PresetStoreError::InvalidFormat(_)) => PresetLibraryV1::empty(),
        Err(error) => return Err(error),
    };

    if let Some(existing) = library
        .presets
        .iter_mut()
        .find(|preset| preset.name == AUTOSAVE_PRESET_NAME)
    {
        existing.scenario = scenario.clone();
    } else {
        library.presets.push(NamedPresetV1 {
            name: AUTOSAVE_PRESET_NAME.to_string(),
            scenario: scenario.clone(),
        });
    }
    library.active_preset = Some(AUTOSAVE_PRESET_NAME.to_string());

    save_library_atomic(path, &library)
}

pub(crate) fn list_named_presets(path: &Path) -> Result<Vec<PresetMetadata>, PresetStoreError> {
    let library = load_library(path)?;
    let active_name = library.active_preset.as_ref();
    let mut presets = library
        .presets
        .into_iter()
        .map(|preset| PresetMetadata {
            is_active: active_name
                .map(|active| active == &preset.name)
                .unwrap_or(false),
            name: preset.name,
        })
        .collect::<Vec<_>>();
    presets.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(presets)
}

pub(crate) fn save_named_preset(
    path: &Path,
    name: &str,
    scenario: &ScenarioPresetV1,
    overwrite: bool,
) -> Result<SaveNamedPresetOutcome, PresetStoreError> {
    let mut library = match load_library(path) {
        Ok(library) => library,
        Err(PresetStoreError::InvalidFormat(_)) => PresetLibraryV1::empty(),
        Err(error) => return Err(error),
    };

    if let Some(existing) = library
        .presets
        .iter_mut()
        .find(|preset| preset.name == name)
    {
        if !overwrite {
            return Ok(SaveNamedPresetOutcome::Conflict);
        }
        existing.scenario = scenario.clone();
        library.active_preset = Some(name.to_string());
        save_library_atomic(path, &library)?;
        return Ok(SaveNamedPresetOutcome::Overwritten);
    }

    library.presets.push(NamedPresetV1 {
        name: name.to_string(),
        scenario: scenario.clone(),
    });
    library.active_preset = Some(name.to_string());
    save_library_atomic(path, &library)?;
    Ok(SaveNamedPresetOutcome::Saved)
}

pub(crate) fn load_named_preset(
    path: &Path,
    name: &str,
) -> Result<Option<ScenarioPresetV1>, PresetStoreError> {
    let mut library = load_library(path)?;
    let scenario = library
        .presets
        .iter()
        .find(|preset| preset.name == name)
        .map(|preset| preset.scenario.clone());

    if scenario.is_some() {
        library.active_preset = Some(name.to_string());
        save_library_atomic(path, &library)?;
    }

    Ok(scenario)
}

pub(crate) fn delete_named_preset(
    path: &Path,
    name: &str,
) -> Result<DeleteNamedPresetOutcome, PresetStoreError> {
    let mut library = load_library(path)?;
    let initial_len = library.presets.len();
    library.presets.retain(|preset| preset.name != name);

    if library.presets.len() == initial_len {
        return Ok(DeleteNamedPresetOutcome::NotFound);
    }

    if library
        .active_preset
        .as_ref()
        .map(|active| active == name)
        .unwrap_or(false)
    {
        library.active_preset = None;
    }

    save_library_atomic(path, &library)?;
    Ok(DeleteNamedPresetOutcome::Deleted)
}

#[allow(dead_code)]
pub(crate) fn export_library(path: &Path, export_path: &Path) -> Result<(), PresetStoreError> {
    let library = load_library(path)?;
    save_library_atomic(export_path, &library)
}

#[allow(dead_code)]
pub(crate) fn import_library(path: &Path, import_path: &Path) -> Result<(), PresetStoreError> {
    let import_contents = fs::read_to_string(import_path).map_err(|error| {
        PresetStoreError::Io(format!(
            "failed to read import file '{}': {error}",
            import_path.display()
        ))
    })?;

    let import_library: PresetLibraryV1 =
        serde_json::from_str(&import_contents).map_err(|error| {
            PresetStoreError::InvalidFormat(format!(
                "invalid import file '{}': {error}",
                import_path.display()
            ))
        })?;

    let candidate = validate_import(import_library, import_path)?;
    save_library_atomic(path, &candidate)
}

#[allow(dead_code)]
fn validate_import(
    library: PresetLibraryV1,
    import_path: &Path,
) -> Result<PresetLibraryV1, PresetStoreError> {
    if library.version != PRESET_FILE_VERSION {
        return Err(PresetStoreError::InvalidFormat(format!(
            "unsupported import file version {} in '{}'",
            library.version,
            import_path.display()
        )));
    }

    let mut names = HashSet::with_capacity(library.presets.len());
    for preset in &library.presets {
        if preset.name.trim().is_empty() {
            return Err(PresetStoreError::InvalidFormat(format!(
                "preset names must not be empty in '{}'",
                import_path.display()
            )));
        }
        if preset.name.trim() != preset.name {
            return Err(PresetStoreError::InvalidFormat(format!(
                "preset names must not have surrounding whitespace in '{}'",
                import_path.display()
            )));
        }
        if !names.insert(preset.name.clone()) {
            return Err(PresetStoreError::InvalidFormat(format!(
                "duplicate preset name '{}' in '{}'",
                preset.name,
                import_path.display()
            )));
        }
    }

    if let Some(active_name) = library.active_preset.as_ref() {
        if active_name.trim().is_empty() {
            return Err(PresetStoreError::InvalidFormat(format!(
                "active preset must not be empty in '{}'",
                import_path.display()
            )));
        }
        if active_name.trim() != active_name {
            return Err(PresetStoreError::InvalidFormat(format!(
                "active preset must not have surrounding whitespace in '{}'",
                import_path.display()
            )));
        }
        if !names.contains(active_name) {
            return Err(PresetStoreError::InvalidFormat(format!(
                "active preset '{}' not found in import file '{}'",
                active_name,
                import_path.display()
            )));
        }
    }

    Ok(library)
}

fn load_library(path: &Path) -> Result<PresetLibraryV1, PresetStoreError> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PresetLibraryV1::empty())
        }
        Err(error) => {
            return Err(PresetStoreError::Io(format!(
                "failed to read presets file '{}': {error}",
                path.display()
            )))
        }
    };

    let library: PresetLibraryV1 = serde_json::from_str(&contents).map_err(|error| {
        PresetStoreError::InvalidFormat(format!(
            "invalid preset file '{}': {error}",
            path.display()
        ))
    })?;

    if library.version != PRESET_FILE_VERSION {
        return Err(PresetStoreError::InvalidFormat(format!(
            "unsupported preset file version {} in '{}'",
            library.version,
            path.display()
        )));
    }

    Ok(library)
}

fn save_library_atomic(path: &Path, library: &PresetLibraryV1) -> Result<(), PresetStoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PresetStoreError::Io(format!(
                "failed to create presets directory '{}': {error}",
                parent.display()
            ))
        })?;
    }

    let serialized = serde_json::to_string_pretty(library).map_err(|error| {
        PresetStoreError::Io(format!("failed to serialize presets to json: {error}"))
    })?;

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let temp_path = path.with_extension(format!("json.tmp.{nanos}"));
    let mut temp_file = File::create(&temp_path).map_err(|error| {
        PresetStoreError::Io(format!(
            "failed to create temp presets file '{}': {error}",
            temp_path.display()
        ))
    })?;
    temp_file
        .write_all(serialized.as_bytes())
        .map_err(|error| {
            PresetStoreError::Io(format!(
                "failed to write temp presets file '{}': {error}",
                temp_path.display()
            ))
        })?;
    temp_file.sync_all().map_err(|error| {
        PresetStoreError::Io(format!(
            "failed to flush temp presets file '{}': {error}",
            temp_path.display()
        ))
    })?;

    replace_file(&temp_path, path)?;
    Ok(())
}

fn replace_file(temp_path: &Path, target_path: &Path) -> Result<(), PresetStoreError> {
    match fs::rename(temp_path, target_path) {
        Ok(()) => Ok(()),
        Err(first_error) => {
            if target_path.exists() {
                fs::remove_file(target_path).map_err(|remove_error| {
                    let _ = fs::remove_file(temp_path);
                    PresetStoreError::Io(format!(
                        "failed to replace presets file '{}': {first_error}; remove failed: {remove_error}",
                        target_path.display()
                    ))
                })?;
                fs::rename(temp_path, target_path).map_err(|rename_error| {
                    let _ = fs::remove_file(temp_path);
                    PresetStoreError::Io(format!(
                        "failed to move temp presets file '{}' to '{}': {rename_error}",
                        temp_path.display(),
                        target_path.display()
                    ))
                })
            } else {
                let _ = fs::remove_file(temp_path);
                Err(PresetStoreError::Io(format!(
                    "failed to move temp presets file '{}' to '{}': {first_error}",
                    temp_path.display(),
                    target_path.display()
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_test_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("sim_ui_preset_tests_{label}_{nanos}"))
    }

    #[test]
    fn load_missing_file_returns_empty_library() {
        let path = unique_test_path("missing").join(PRESETS_FILE_NAME);
        let loaded = load_library(&path).expect("missing file should be treated as empty");
        assert_eq!(loaded.version, PRESET_FILE_VERSION);
        assert!(loaded.presets.is_empty());
        assert!(loaded.active_preset.is_none());
    }

    #[test]
    fn malformed_json_is_reported_as_recoverable_error() {
        let path = unique_test_path("malformed").join(PRESETS_FILE_NAME);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("test directory should be creatable");
        }
        fs::write(&path, "{ definitely-not-json ").expect("test fixture should be written");

        let result = load_library(&path);
        assert!(matches!(result, Err(PresetStoreError::InvalidFormat(_))));
    }

    #[test]
    fn save_and_load_round_trip() {
        let path = unique_test_path("round_trip").join(PRESETS_FILE_NAME);
        let defaults = AppDefaults::new();
        let scenario = ScenarioPresetV1::from_defaults(&defaults);

        save_autosave_preset(&path, &scenario).expect("save should succeed");
        let loaded = load_active_preset(&path)
            .expect("load should succeed")
            .expect("active preset should exist");
        assert_eq!(loaded, scenario);
    }

    #[test]
    fn apply_to_defaults_normalizes_out_of_range_values() {
        let mut defaults = AppDefaults::new();
        let mut scenario = ScenarioPresetV1::from_defaults(&defaults);
        scenario.num_riders = 0;
        scenario.request_window_hours = 0;
        scenario.simulation_duration_hours = 500;
        scenario.min_trip_km = 300.0;
        scenario.max_trip_km = 0.0;
        scenario.rider_cancel_min_mins = 900;
        scenario.rider_cancel_max_mins = 0;
        scenario.commission_rate = -1.0;
        scenario.accept_probability = 2.5;
        scenario.driver_pickup_distance_penalty = 2.0;
        scenario.base_speed_kmh = 5.0;
        scenario.start_month = 99;
        scenario.start_minute = 99;
        scenario.osrm_endpoint = "   ".to_string();

        scenario.apply_to_defaults(&mut defaults);

        assert_eq!(defaults.num_riders, 1);
        assert_eq!(defaults.request_window_hours, 1);
        assert_eq!(defaults.simulation_duration_hours, 168);
        assert_eq!(defaults.min_trip_km, 100.0);
        assert_eq!(defaults.max_trip_km, 100.0);
        assert_eq!(defaults.rider_cancel_min_mins, 600);
        assert_eq!(defaults.rider_cancel_max_mins, 600);
        assert_eq!(defaults.commission_rate, 0.0);
        assert_eq!(defaults.accept_probability, 1.0);
        assert_eq!(defaults.driver_pickup_distance_penalty, 0.0);
        assert_eq!(defaults.base_speed_kmh, 10.0);
        assert_eq!(defaults.start_month, 12);
        assert_eq!(defaults.start_minute, 59);
        assert_eq!(defaults.osrm_endpoint, "http://localhost:5000");
    }

    #[test]
    fn save_named_preset_returns_conflict_without_overwrite() {
        let path = unique_test_path("named_conflict").join(PRESETS_FILE_NAME);
        let defaults = AppDefaults::new();
        let scenario = ScenarioPresetV1::from_defaults(&defaults);

        let first = save_named_preset(&path, "morning", &scenario, false)
            .expect("initial named save should succeed");
        assert_eq!(first, SaveNamedPresetOutcome::Saved);

        let conflict = save_named_preset(&path, "morning", &scenario, false)
            .expect("duplicate save should return conflict");
        assert_eq!(conflict, SaveNamedPresetOutcome::Conflict);
    }

    #[test]
    fn overwrite_replaces_payload_and_sets_active_name() {
        let path = unique_test_path("overwrite").join(PRESETS_FILE_NAME);
        let defaults = AppDefaults::new();
        let first = ScenarioPresetV1::from_defaults(&defaults);
        let mut second = first.clone();
        second.num_riders = first.num_riders.saturating_add(99);

        let first_result =
            save_named_preset(&path, "weekday", &first, false).expect("first save should succeed");
        assert_eq!(first_result, SaveNamedPresetOutcome::Saved);

        let overwrite = save_named_preset(&path, "weekday", &second, true)
            .expect("overwrite save should succeed");
        assert_eq!(overwrite, SaveNamedPresetOutcome::Overwritten);

        let loaded = load_named_preset(&path, "weekday")
            .expect("load should succeed")
            .expect("saved preset should exist");
        assert_eq!(loaded, second);

        let metadata = list_named_presets(&path).expect("list should succeed");
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata[0].name, "weekday");
        assert!(metadata[0].is_active);
    }

    #[test]
    fn delete_removes_named_preset_from_disk() {
        let path = unique_test_path("delete").join(PRESETS_FILE_NAME);
        let defaults = AppDefaults::new();
        let scenario = ScenarioPresetV1::from_defaults(&defaults);

        save_named_preset(&path, "to-delete", &scenario, false).expect("save should succeed");

        let result = delete_named_preset(&path, "to-delete").expect("delete should succeed");
        assert_eq!(result, DeleteNamedPresetOutcome::Deleted);

        let metadata = list_named_presets(&path).expect("list should succeed");
        assert!(metadata.is_empty());
    }

    #[test]
    fn deleting_active_named_preset_clears_active_marker() {
        let path = unique_test_path("delete_active").join(PRESETS_FILE_NAME);
        let defaults = AppDefaults::new();
        let scenario = ScenarioPresetV1::from_defaults(&defaults);

        save_named_preset(&path, "active", &scenario, false).expect("save should succeed");
        let deleted = delete_named_preset(&path, "active").expect("delete should succeed");
        assert_eq!(deleted, DeleteNamedPresetOutcome::Deleted);

        let metadata = list_named_presets(&path).expect("list should succeed");
        assert!(metadata.is_empty());
        let loaded_active = load_active_preset(&path).expect("active load should succeed");
        assert!(loaded_active.is_none());
    }

    #[test]
    fn import_with_unsupported_version_keeps_existing_library_unchanged() {
        let store_path = unique_test_path("import_invalid_version").join(PRESETS_FILE_NAME);
        let import_path = unique_test_path("import_invalid_version_src").join("library.json");
        let defaults = AppDefaults::new();
        let baseline = ScenarioPresetV1::from_defaults(&defaults);

        save_named_preset(&store_path, "baseline", &baseline, false)
            .expect("baseline save should succeed");

        if let Some(parent) = import_path.parent() {
            fs::create_dir_all(parent).expect("import fixture directory should be creatable");
        }
        fs::write(
            &import_path,
            r#"{
  "version": 99,
  "active_preset": null,
  "presets": []
}"#,
        )
        .expect("import fixture should be written");

        let result = import_library(&store_path, &import_path);
        assert!(matches!(result, Err(PresetStoreError::InvalidFormat(_))));

        let listed = list_named_presets(&store_path).expect("list should still succeed");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "baseline");
        assert!(listed[0].is_active);
        let loaded = load_named_preset(&store_path, "baseline")
            .expect("load should succeed")
            .expect("baseline preset should still exist");
        assert_eq!(loaded, baseline);
    }

    #[test]
    fn import_valid_library_replaces_existing_library() {
        let store_path = unique_test_path("import_valid_replace").join(PRESETS_FILE_NAME);
        let import_path = unique_test_path("import_valid_replace_src").join("library.json");
        let defaults = AppDefaults::new();
        let old_scenario = ScenarioPresetV1::from_defaults(&defaults);
        let mut imported_scenario = old_scenario.clone();
        imported_scenario.num_drivers = imported_scenario.num_drivers.saturating_add(77);

        save_named_preset(&store_path, "existing", &old_scenario, false)
            .expect("existing save should succeed");

        let import_library_payload = PresetLibraryV1 {
            version: PRESET_FILE_VERSION,
            active_preset: Some("imported".to_string()),
            presets: vec![NamedPresetV1 {
                name: "imported".to_string(),
                scenario: imported_scenario.clone(),
            }],
        };

        if let Some(parent) = import_path.parent() {
            fs::create_dir_all(parent).expect("import fixture directory should be creatable");
        }
        let serialized = serde_json::to_string_pretty(&import_library_payload)
            .expect("import payload should serialize");
        fs::write(&import_path, serialized).expect("import payload should be written");

        import_library(&store_path, &import_path).expect("import should succeed");

        let listed = list_named_presets(&store_path).expect("list should succeed");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "imported");
        assert!(listed[0].is_active);
        let loaded = load_named_preset(&store_path, "imported")
            .expect("load should succeed")
            .expect("imported preset should exist");
        assert_eq!(loaded, imported_scenario);
    }

    #[test]
    fn strict_import_validation_rejects_invalid_payloads_without_mutating_store() {
        let store_path = unique_test_path("strict_validation_store").join(PRESETS_FILE_NAME);
        let import_path = unique_test_path("strict_validation_import").join("library.json");
        let defaults = AppDefaults::new();
        let baseline_scenario = ScenarioPresetV1::from_defaults(&defaults);

        save_named_preset(&store_path, "baseline", &baseline_scenario, false)
            .expect("baseline save should succeed");

        let invalid_payloads = vec![
            (
                "unknown_field",
                r#"{
  "version": 1,
  "active_preset": null,
  "presets": [],
  "unexpected": true
}"#
                .to_string(),
            ),
            (
                "trimmed_name",
                serde_json::to_string_pretty(&PresetLibraryV1 {
                    version: PRESET_FILE_VERSION,
                    active_preset: Some(" baseline ".to_string()),
                    presets: vec![NamedPresetV1 {
                        name: " baseline ".to_string(),
                        scenario: baseline_scenario.clone(),
                    }],
                })
                .expect("payload should serialize"),
            ),
            (
                "duplicate_name",
                serde_json::to_string_pretty(&PresetLibraryV1 {
                    version: PRESET_FILE_VERSION,
                    active_preset: Some("dup".to_string()),
                    presets: vec![
                        NamedPresetV1 {
                            name: "dup".to_string(),
                            scenario: baseline_scenario.clone(),
                        },
                        NamedPresetV1 {
                            name: "dup".to_string(),
                            scenario: baseline_scenario.clone(),
                        },
                    ],
                })
                .expect("payload should serialize"),
            ),
            (
                "missing_active_reference",
                serde_json::to_string_pretty(&PresetLibraryV1 {
                    version: PRESET_FILE_VERSION,
                    active_preset: Some("missing".to_string()),
                    presets: vec![NamedPresetV1 {
                        name: "present".to_string(),
                        scenario: baseline_scenario.clone(),
                    }],
                })
                .expect("payload should serialize"),
            ),
        ];

        for (label, payload) in invalid_payloads {
            if let Some(parent) = import_path.parent() {
                fs::create_dir_all(parent).expect("import fixture directory should be creatable");
            }
            fs::write(&import_path, payload).expect("import fixture should be written");

            let result = import_library(&store_path, &import_path);
            assert!(
                matches!(result, Err(PresetStoreError::InvalidFormat(_))),
                "case '{label}' should reject invalid import"
            );

            let listed = list_named_presets(&store_path).expect("list should still succeed");
            assert_eq!(listed.len(), 1, "case '{label}' should keep preset count");
            assert_eq!(
                listed[0].name, "baseline",
                "case '{label}' should keep name"
            );
            assert!(
                listed[0].is_active,
                "case '{label}' should keep active marker"
            );

            let loaded = load_named_preset(&store_path, "baseline")
                .expect("load should succeed")
                .expect("baseline should still exist");
            assert_eq!(
                loaded, baseline_scenario,
                "case '{label}' should keep baseline data"
            );
        }
    }
}
