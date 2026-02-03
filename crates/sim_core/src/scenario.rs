//! Scenario setup: spawn riders and drivers with random positions and request times.
//!
//! Uses a geographic bounding box to sample random H3 cells (resolution 9) and
//! spreads rider request events over a configurable time window.

use bevy_ecs::prelude::{Resource, World};
use h3o::{CellIndex, LatLng, Resolution};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::clock::{EventKind, EventSubject, SimulationClock};
use crate::ecs::{Driver, DriverState, Position, Rider, RiderState};
use crate::spatial::GeoIndex;
use crate::telemetry::{SimSnapshotConfig, SimSnapshots, SimTelemetry};

/// Default bounding box: San Francisco Bay Area (approx).
const DEFAULT_LAT_MIN: f64 = 37.6;
const DEFAULT_LAT_MAX: f64 = 37.85;
const DEFAULT_LNG_MIN: f64 = -122.55;
const DEFAULT_LNG_MAX: f64 = -122.35;

/// Default time window for rider requests: 1 hour (simulation ms).
const DEFAULT_REQUEST_WINDOW_MS: u64 = 60 * 60 * 1000;

/// Max H3 grid distance (cells) for matching rider to driver. 0 = same cell only.
#[derive(Debug, Clone, Copy, Default, Resource)]
pub struct MatchRadius(pub u32);

/// Parameters for building a scenario.
#[derive(Debug, Clone)]
pub struct ScenarioParams {
    pub num_riders: usize,
    pub num_drivers: usize,
    /// Random seed for reproducibility (optional; if None, uses thread rng).
    pub seed: Option<u64>,
    /// Bounding box for random positions (lat/lng degrees).
    pub lat_min: f64,
    pub lat_max: f64,
    pub lng_min: f64,
    pub lng_max: f64,
    /// Time window in simulation ms: rider RequestInbound times are uniform in [0, request_window_ms].
    pub request_window_ms: u64,
    /// Max H3 grid distance for matching (0 = same cell only).
    pub match_radius: u32,
    /// Min/max trip length in H3 cells (movement uses 1 min per cell; e.g. 5..60 = 5 min to 1h).
    pub min_trip_cells: u32,
    pub max_trip_cells: u32,
}

impl Default for ScenarioParams {
    fn default() -> Self {
        Self {
            num_riders: 500,
            num_drivers: 100,
            seed: None,
            lat_min: DEFAULT_LAT_MIN,
            lat_max: DEFAULT_LAT_MAX,
            lng_min: DEFAULT_LNG_MIN,
            lng_max: DEFAULT_LNG_MAX,
            request_window_ms: DEFAULT_REQUEST_WINDOW_MS,
            match_radius: 0,
            min_trip_cells: 5,
            max_trip_cells: 60,
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

    /// Match riders to drivers within this H3 grid distance (0 = same cell only).
    pub fn with_match_radius(mut self, radius: u32) -> Self {
        self.match_radius = radius;
        self
    }

    /// Trip length in H3 cells: min..=max (movement uses 1 min per cell; 60 cells ≈ 1h).
    pub fn with_trip_duration_cells(mut self, min_cells: u32, max_cells: u32) -> Self {
        self.min_trip_cells = min_cells;
        self.max_trip_cells = max_cells;
        self
    }
}

/// Sample a random H3 cell (resolution 9) within the given lat/lng bounds.
fn random_cell_in_bounds<R: Rng>(rng: &mut R, lat_min: f64, lat_max: f64, lng_min: f64, lng_max: f64) -> CellIndex {
    let lat = rng.gen_range(lat_min..=lat_max);
    let lng = rng.gen_range(lng_min..=lng_max);
    let coord = LatLng::new(lat, lng).expect("valid lat/lng");
    coord.to_cell(Resolution::Nine)
}

/// Pick a random destination within [min_cells, max_cells] H3 distance from pickup.
fn random_destination<R: Rng>(
    rng: &mut R,
    pickup: CellIndex,
    geo: &GeoIndex,
    min_cells: u32,
    max_cells: u32,
) -> Option<CellIndex> {
    let max_cells = max_cells.max(min_cells);
    let disk = geo.grid_disk(pickup, max_cells);
    let candidates: Vec<CellIndex> = disk
        .into_iter()
        .filter(|c| {
            pickup
                .grid_distance(*c)
                .map(|d| d >= min_cells as i32 && d <= max_cells as i32)
                .unwrap_or(false)
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let i = rng.gen_range(0..candidates.len());
    Some(candidates[i])
}

/// Populates `world` with clock, telemetry, riders, drivers, and scheduled RequestInbound events.
/// Caller must have already created `world`; this inserts resources and spawns entities.
pub fn build_scenario(world: &mut World, params: ScenarioParams) {
    world.insert_resource(SimulationClock::default());
    world.insert_resource(SimTelemetry::default());
    world.insert_resource(SimSnapshotConfig::default());
    world.insert_resource(SimSnapshots::default());
    world.insert_resource(MatchRadius(params.match_radius));

    let mut rng: StdRng = match params.seed {
        Some(seed) => StdRng::seed_from_u64(seed),
        None => StdRng::from_entropy(),
    };

    let geo = GeoIndex::default();
    let lat_min = params.lat_min;
    let lat_max = params.lat_max;
    let lng_min = params.lng_min;
    let lng_max = params.lng_max;
    let request_window_ms = params.request_window_ms;
    let min_trip = params.min_trip_cells;
    let max_trip = params.max_trip_cells;

    let mut rider_entities = Vec::with_capacity(params.num_riders);

    for _ in 0..params.num_riders {
        let cell = random_cell_in_bounds(&mut rng, lat_min, lat_max, lng_min, lng_max);
        let destination = random_destination(&mut rng, cell, &geo, min_trip, max_trip);

        let entity = world
            .spawn((
                Rider {
                    state: RiderState::Requesting,
                    matched_driver: None,
                    destination,
                    requested_at: None,
                },
                Position(cell),
            ))
            .id();
        rider_entities.push(entity);
    }

    for _ in 0..params.num_drivers {
        let cell = random_cell_in_bounds(&mut rng, lat_min, lat_max, lng_min, lng_max);
        world.spawn((
            Driver {
                state: DriverState::Idle,
                matched_rider: None,
            },
            Position(cell),
        ));
    }

    let mut clock = world.resource_mut::<SimulationClock>();
    for rider_entity in rider_entities {
        let at_ms = rng.gen_range(0..=request_window_ms);
        clock.schedule_at(at_ms, EventKind::RequestInbound, Some(EventSubject::Rider(rider_entity)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_scenario_spawns_riders_and_drivers() {
        let mut world = World::new();
        build_scenario(
            &mut world,
            ScenarioParams {
                num_riders: 10,
                num_drivers: 3,
                seed: Some(42),
                ..Default::default()
            },
        );

        let rider_count = world.query::<&Rider>().iter(&world).count();
        let driver_count = world.query::<&Driver>().iter(&world).count();
        assert_eq!(rider_count, 10);
        assert_eq!(driver_count, 3);

        let clock = world.resource::<SimulationClock>();
        assert_eq!(clock.pending_event_count(), 10, "one RequestInbound per rider");
    }
}
