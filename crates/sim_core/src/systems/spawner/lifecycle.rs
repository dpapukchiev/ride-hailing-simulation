use bevy_ecs::prelude::Commands;

use crate::clock::{EventKind, SimulationClock};
use crate::spawner::{DriverSpawner, RiderSpawner, SpawnWeighting};

use super::{spawn_driver, spawn_rider, MaybeOsrmSpawnMetrics};

pub(super) fn initialize_rider_spawner(
    spawner: &mut RiderSpawner,
    commands: &mut Commands,
    clock: &mut SimulationClock,
    current_time_ms: u64,
    weighting: Option<&SpawnWeighting>,
    osrm_metrics: MaybeOsrmSpawnMetrics<'_>,
) {
    if !spawner.initialized() {
        spawner.set_initialized(true);

        for _ in 0..spawner.config.initial_count {
            spawn_rider(
                commands,
                clock,
                spawner,
                current_time_ms,
                weighting,
                osrm_metrics,
            );
            spawner.increment_spawned_count();
        }

        if spawner.should_spawn(spawner.next_spawn_time_ms()) {
            clock.schedule_at(spawner.next_spawn_time_ms(), EventKind::SpawnRider, None);
        }
    }
}

pub(super) fn initialize_driver_spawner(
    spawner: &mut DriverSpawner,
    commands: &mut Commands,
    clock: &mut SimulationClock,
    current_time_ms: u64,
    weighting: Option<&SpawnWeighting>,
    osrm_metrics: MaybeOsrmSpawnMetrics<'_>,
) {
    if !spawner.initialized() {
        spawner.set_initialized(true);

        for _ in 0..spawner.config.initial_count {
            spawn_driver(commands, spawner, current_time_ms, weighting, osrm_metrics);
            spawner.increment_spawned_count();
        }

        if spawner.should_spawn(spawner.next_spawn_time_ms()) {
            clock.schedule_at(spawner.next_spawn_time_ms(), EventKind::SpawnDriver, None);
        }
    }
}

pub(super) fn process_rider_spawner_event(
    spawner: &mut RiderSpawner,
    commands: &mut Commands,
    clock: &mut SimulationClock,
    current_time_ms: u64,
    weighting: Option<&SpawnWeighting>,
    osrm_metrics: MaybeOsrmSpawnMetrics<'_>,
) {
    if let Some(start_time) = spawner.config.start_time_ms {
        if current_time_ms < start_time {
            spawner.set_next_spawn_time_ms(start_time);
            clock.schedule_at(start_time, EventKind::SpawnRider, None);
            return;
        }
    }

    if spawner.should_spawn(current_time_ms) {
        spawn_rider(
            commands,
            clock,
            spawner,
            current_time_ms,
            weighting,
            osrm_metrics,
        );

        spawner.advance(current_time_ms);

        if spawner.should_spawn(spawner.next_spawn_time_ms()) {
            clock.schedule_at(spawner.next_spawn_time_ms(), EventKind::SpawnRider, None);
        }
    }
}

pub(super) fn process_driver_spawner_event(
    spawner: &mut DriverSpawner,
    commands: &mut Commands,
    clock: &mut SimulationClock,
    current_time_ms: u64,
    weighting: Option<&SpawnWeighting>,
    osrm_metrics: MaybeOsrmSpawnMetrics<'_>,
) {
    if let Some(start_time) = spawner.config.start_time_ms {
        if current_time_ms < start_time {
            spawner.set_next_spawn_time_ms(start_time);
            clock.schedule_at(start_time, EventKind::SpawnDriver, None);
            return;
        }
    }

    if spawner.should_spawn(current_time_ms) {
        spawn_driver(commands, spawner, current_time_ms, weighting, osrm_metrics);

        spawner.advance(current_time_ms);

        if spawner.should_spawn(spawner.next_spawn_time_ms()) {
            clock.schedule_at(spawner.next_spawn_time_ms(), EventKind::SpawnDriver, None);
        }
    }
}
