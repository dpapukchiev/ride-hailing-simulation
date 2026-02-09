//! Spawner systems: react to spawn events and create riders/drivers dynamically.

mod common;
mod entity_spawn;
mod lifecycle;
#[cfg(feature = "osrm")]
mod osrm;

use bevy_ecs::prelude::{Commands, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, SimulationClock, ONE_MIN_MS};
use crate::scenario::BatchMatchingConfig;
use crate::spawner::{DriverSpawner, RiderSpawner, SpawnWeighting};

#[cfg(feature = "osrm")]
use crate::routing::osrm_spawn::OsrmSpawnClient;
#[cfg(feature = "osrm")]
use crate::telemetry::OsrmSpawnTelemetry;

use common::{create_spawn_rng, resolve_spawn_location};
use entity_spawn::{spawn_driver, spawn_rider};
use lifecycle::{
    initialize_driver_spawner, initialize_rider_spawner, process_driver_spawner_event,
    process_rider_spawner_event,
};

#[cfg(feature = "osrm")]
type MaybeOsrmSpawnClient<'a> = Option<&'a OsrmSpawnClient>;
#[cfg(not(feature = "osrm"))]
type MaybeOsrmSpawnClient<'a> = ();

#[cfg(feature = "osrm")]
type MaybeOsrmSpawnMetrics<'a> = Option<&'a OsrmSpawnTelemetry>;
#[cfg(not(feature = "osrm"))]
type MaybeOsrmSpawnMetrics<'a> = ();

pub fn simulation_started_system(
    mut commands: Commands,
    mut clock: ResMut<SimulationClock>,
    batch_config: Option<Res<BatchMatchingConfig>>,
    rider_spawner: Option<ResMut<RiderSpawner>>,
    driver_spawner: Option<ResMut<DriverSpawner>>,
    spawn_weighting: Option<Res<SpawnWeighting>>,
    #[cfg(feature = "osrm")] osrm_spawn_metrics: Option<Res<OsrmSpawnTelemetry>>,
    event: Res<CurrentEvent>,
) {
    if event.0.kind != EventKind::SimulationStarted {
        return;
    }

    let current_time_ms = clock.now();
    let weighting = spawn_weighting.as_deref();
    #[cfg(feature = "osrm")]
    let osrm_spawn_metrics_ref: MaybeOsrmSpawnMetrics<'_> = osrm_spawn_metrics.as_deref();
    #[cfg(not(feature = "osrm"))]
    let osrm_spawn_metrics_ref: MaybeOsrmSpawnMetrics<'_> = ();

    if let Some(cfg) = batch_config.as_deref() {
        if cfg.enabled {
            clock.schedule_at(0, EventKind::BatchMatchRun, None);
        }
    }

    if let Some(mut spawner) = rider_spawner {
        initialize_rider_spawner(
            &mut spawner,
            &mut commands,
            &mut clock,
            current_time_ms,
            weighting,
            osrm_spawn_metrics_ref,
        );
    }

    if let Some(mut spawner) = driver_spawner {
        initialize_driver_spawner(
            &mut spawner,
            &mut commands,
            &mut clock,
            current_time_ms,
            weighting,
            osrm_spawn_metrics_ref,
        );
    }

    clock.schedule_in(5 * ONE_MIN_MS, EventKind::CheckDriverOffDuty, None);
}

pub fn rider_spawner_system(
    mut commands: Commands,
    mut clock: ResMut<SimulationClock>,
    mut spawner: ResMut<RiderSpawner>,
    spawn_weighting: Option<Res<SpawnWeighting>>,
    #[cfg(feature = "osrm")] osrm_spawn_metrics: Option<Res<OsrmSpawnTelemetry>>,
    event: Res<CurrentEvent>,
) {
    if event.0.kind != EventKind::SpawnRider {
        return;
    }

    let current_time_ms = clock.now();
    let weighting = spawn_weighting.as_deref();
    #[cfg(feature = "osrm")]
    let osrm_spawn_metrics_ref: MaybeOsrmSpawnMetrics<'_> = osrm_spawn_metrics.as_deref();
    #[cfg(not(feature = "osrm"))]
    let osrm_spawn_metrics_ref: MaybeOsrmSpawnMetrics<'_> = ();
    process_rider_spawner_event(
        &mut spawner,
        &mut commands,
        &mut clock,
        current_time_ms,
        weighting,
        osrm_spawn_metrics_ref,
    );
}

pub fn driver_spawner_system(
    mut commands: Commands,
    mut clock: ResMut<SimulationClock>,
    mut spawner: ResMut<DriverSpawner>,
    spawn_weighting: Option<Res<SpawnWeighting>>,
    #[cfg(feature = "osrm")] osrm_spawn_metrics: Option<Res<OsrmSpawnTelemetry>>,
    event: Res<CurrentEvent>,
) {
    if event.0.kind != EventKind::SpawnDriver {
        return;
    }

    let current_time_ms = clock.now();
    let weighting = spawn_weighting.as_deref();
    #[cfg(feature = "osrm")]
    let osrm_spawn_metrics_ref: MaybeOsrmSpawnMetrics<'_> = osrm_spawn_metrics.as_deref();
    #[cfg(not(feature = "osrm"))]
    let osrm_spawn_metrics_ref: MaybeOsrmSpawnMetrics<'_> = ();
    process_driver_spawner_event(
        &mut spawner,
        &mut commands,
        &mut clock,
        current_time_ms,
        weighting,
        osrm_spawn_metrics_ref,
    );
}
