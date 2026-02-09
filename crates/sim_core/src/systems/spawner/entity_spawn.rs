use bevy_ecs::prelude::Commands;
use rand::Rng;

use crate::clock::{EventKind, EventSubject, SimulationClock, ONE_HOUR_MS};
use crate::ecs::{
    Browsing, Driver, DriverEarnings, DriverFatigue, GeoPosition, Idle, Position, Rider,
};
use crate::scenario::random_destination;
use crate::spatial::GeoIndex;
use crate::spawner::{DriverSpawner, RiderSpawner, SpawnWeighting};

use super::{create_spawn_rng, resolve_spawn_location, MaybeOsrmSpawnMetrics};

pub(super) fn spawn_rider(
    commands: &mut Commands,
    clock: &mut SimulationClock,
    spawner: &mut RiderSpawner,
    current_time_ms: u64,
    weighting: Option<&SpawnWeighting>,
    osrm_metrics: MaybeOsrmSpawnMetrics<'_>,
) -> bevy_ecs::prelude::Entity {
    let mut rng = create_spawn_rng(spawner.config.seed, spawner.spawned_count());

    let spawn_location = resolve_spawn_location(
        &mut rng,
        weighting,
        |w, rng| w.sample_rider_cell(rng),
        spawner.config.lat_min,
        spawner.config.lat_max,
        spawner.config.lng_min,
        spawner.config.lng_max,
        spawner.osrm_spawn_client(),
        osrm_metrics,
    );
    let position = spawn_location.cell;

    let geo = GeoIndex::default();
    let destination = random_destination(
        &mut rng,
        position,
        &geo,
        spawner.config.min_trip_cells,
        spawner.config.max_trip_cells,
        spawner.config.lat_min,
        spawner.config.lat_max,
        spawner.config.lng_min,
        spawner.config.lng_max,
    );

    let rider_entity = commands
        .spawn((
            Rider {
                matched_driver: None,
                assigned_trip: None,
                destination: Some(destination),
                requested_at: Some(current_time_ms),
                quote_rejections: 0,
                accepted_fare: None,
                last_rejection_reason: None,
            },
            Browsing,
            Position(position),
            GeoPosition(spawn_location.geo),
        ))
        .id();

    clock.schedule_in_secs(
        1,
        EventKind::ShowQuote,
        Some(EventSubject::Rider(rider_entity)),
    );

    rider_entity
}

pub(super) fn spawn_driver(
    commands: &mut Commands,
    spawner: &mut DriverSpawner,
    current_time_ms: u64,
    weighting: Option<&SpawnWeighting>,
    osrm_metrics: MaybeOsrmSpawnMetrics<'_>,
) {
    let mut rng = create_spawn_rng(spawner.config.seed, spawner.spawned_count());

    let spawn_location = resolve_spawn_location(
        &mut rng,
        weighting,
        |w, rng| w.sample_driver_cell(rng),
        spawner.config.lat_min,
        spawner.config.lat_max,
        spawner.config.lng_min,
        spawner.config.lng_max,
        spawner.osrm_spawn_client(),
        osrm_metrics,
    );

    let daily_earnings_target = rng.gen_range(100.0..=300.0);
    let fatigue_hours = rng.gen_range(8.0..=12.0);
    let fatigue_threshold_ms = (fatigue_hours * ONE_HOUR_MS as f64) as u64;

    commands.spawn((
        Driver {
            matched_rider: None,
            assigned_trip: None,
        },
        Idle,
        Position(spawn_location.cell),
        GeoPosition(spawn_location.geo),
        DriverEarnings {
            daily_earnings: 0.0,
            daily_earnings_target,
            session_start_time_ms: current_time_ms,
            session_end_time_ms: None,
        },
        DriverFatigue {
            fatigue_threshold_ms,
        },
    ));
}
