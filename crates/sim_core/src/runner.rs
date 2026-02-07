//! Simulation runner: advances the clock and routes events into the ECS.
//!
//! Clock progression and event routing happen here, outside systems. Each step
//! pops the next event from [SimulationClock], inserts it as [CurrentEvent],
//! then runs the schedule.

use bevy_ecs::prelude::Res;
use bevy_ecs::prelude::{Schedule, World};
use bevy_ecs::schedule::{apply_deferred, IntoSystemConfigs};

use crate::clock::{CurrentEvent, Event, EventKind, SimulationClock};
use crate::profiling::EventMetrics;
use crate::scenario::SimulationEndTimeMs;
use crate::systems::{
    batch_matching::batch_matching_system,
    driver_decision::driver_decision_system,
    driver_offduty::driver_offduty_check_system,
    match_accepted::match_accepted_system,
    matching::matching_system,
    movement::movement_system,
    pickup_eta_updated::pickup_eta_updated_system,
    quote_accepted::quote_accepted_system,
    quote_decision::quote_decision_system,
    quote_rejected::quote_rejected_system,
    rider_cancel::rider_cancel_system,
    show_quote::show_quote_system,
    spatial_index::{update_spatial_index_drivers_system, update_spatial_index_riders_system},
    spawner::{driver_spawner_system, rider_spawner_system, simulation_started_system},
    telemetry_snapshot::capture_snapshot_system,
    trip_completed::trip_completed_system,
    trip_started::trip_started_system,
};

// Condition functions for each event kind
fn is_simulation_started(event: Option<Res<CurrentEvent>>) -> bool {
    event
        .map(|e| e.0.kind == EventKind::SimulationStarted)
        .unwrap_or(false)
}

fn is_spawn_rider(event: Option<Res<CurrentEvent>>) -> bool {
    event
        .map(|e| e.0.kind == EventKind::SpawnRider)
        .unwrap_or(false)
}

fn is_spawn_driver(event: Option<Res<CurrentEvent>>) -> bool {
    event
        .map(|e| e.0.kind == EventKind::SpawnDriver)
        .unwrap_or(false)
}

fn is_show_quote(event: Option<Res<CurrentEvent>>) -> bool {
    event
        .map(|e| e.0.kind == EventKind::ShowQuote)
        .unwrap_or(false)
}

fn is_quote_decision(event: Option<Res<CurrentEvent>>) -> bool {
    event
        .map(|e| e.0.kind == EventKind::QuoteDecision)
        .unwrap_or(false)
}

fn is_quote_accepted(event: Option<Res<CurrentEvent>>) -> bool {
    event
        .map(|e| e.0.kind == EventKind::QuoteAccepted)
        .unwrap_or(false)
}

fn is_quote_rejected(event: Option<Res<CurrentEvent>>) -> bool {
    event
        .map(|e| e.0.kind == EventKind::QuoteRejected)
        .unwrap_or(false)
}

fn is_try_match(event: Option<Res<CurrentEvent>>) -> bool {
    event
        .map(|e| e.0.kind == EventKind::TryMatch)
        .unwrap_or(false)
}

fn is_batch_match_run(event: Option<Res<CurrentEvent>>) -> bool {
    event
        .map(|e| e.0.kind == EventKind::BatchMatchRun)
        .unwrap_or(false)
}

fn is_match_accepted(event: Option<Res<CurrentEvent>>) -> bool {
    event
        .map(|e| e.0.kind == EventKind::MatchAccepted)
        .unwrap_or(false)
}

fn is_driver_decision(event: Option<Res<CurrentEvent>>) -> bool {
    event
        .map(|e| e.0.kind == EventKind::DriverDecision)
        .unwrap_or(false)
}

fn is_rider_cancel(event: Option<Res<CurrentEvent>>) -> bool {
    event
        .map(|e| e.0.kind == EventKind::RiderCancel)
        .unwrap_or(false)
}

fn is_move_step(event: Option<Res<CurrentEvent>>) -> bool {
    event
        .map(|e| e.0.kind == EventKind::MoveStep)
        .unwrap_or(false)
}

fn is_pickup_eta_updated(event: Option<Res<CurrentEvent>>) -> bool {
    event
        .map(|e| e.0.kind == EventKind::PickupEtaUpdated)
        .unwrap_or(false)
}

fn is_trip_started(event: Option<Res<CurrentEvent>>) -> bool {
    event
        .map(|e| e.0.kind == EventKind::TripStarted)
        .unwrap_or(false)
}

fn is_trip_completed(event: Option<Res<CurrentEvent>>) -> bool {
    event
        .map(|e| e.0.kind == EventKind::TripCompleted)
        .unwrap_or(false)
}

fn is_check_driver_offduty(event: Option<Res<CurrentEvent>>) -> bool {
    event
        .map(|e| e.0.kind == EventKind::CheckDriverOffDuty)
        .unwrap_or(false)
}

/// Condition: telemetry snapshot interval has elapsed.
fn should_capture_snapshot(
    clock: Option<Res<SimulationClock>>,
    config: Option<Res<crate::telemetry::SimSnapshotConfig>>,
    snapshots: Option<Res<crate::telemetry::SimSnapshots>>,
) -> bool {
    let Some(clock) = clock else {
        return false;
    };
    let Some(config) = config else {
        return false;
    };
    let Some(snapshots) = snapshots else {
        return false;
    };

    let now = clock.now();
    match snapshots.last_snapshot_at {
        None => true,
        Some(last) => now.saturating_sub(last) >= config.interval_ms,
    }
}

/// Runs one simulation step: pops the next event, inserts it as [CurrentEvent], then runs the schedule.
/// Returns `true` if an event was processed, `false` if the clock was empty or if the next event
/// is at or past [SimulationEndTimeMs] (when that resource is present).
pub fn run_next_event(world: &mut World, schedule: &mut Schedule) -> bool {
    let stop_at = world.get_resource::<SimulationEndTimeMs>().map(|e| e.0);
    let next_ts = world
        .get_resource::<SimulationClock>()
        .and_then(|c| c.next_event_time());
    if let (Some(end_ms), Some(ts)) = (stop_at, next_ts) {
        if ts >= end_ms {
            return false;
        }
    }

    let event = match world.resource_mut::<SimulationClock>().pop_next() {
        Some(e) => e,
        None => return false,
    };
    world.insert_resource(CurrentEvent(event));

    // Track event metrics if EventMetrics resource exists
    if let Some(mut metrics) = world.get_resource_mut::<EventMetrics>() {
        metrics.record_event(event.kind);
    }

    schedule.run(world);
    true
}

/// Runs one simulation step and invokes `hook` after the schedule completes.
pub fn run_next_event_with_hook<F>(world: &mut World, schedule: &mut Schedule, mut hook: F) -> bool
where
    F: FnMut(&World, &Event),
{
    let stop_at = world.get_resource::<SimulationEndTimeMs>().map(|e| e.0);
    let next_ts = world
        .get_resource::<SimulationClock>()
        .and_then(|c| c.next_event_time());
    if let (Some(end_ms), Some(ts)) = (stop_at, next_ts) {
        if ts >= end_ms {
            return false;
        }
    }

    let event = match world.resource_mut::<SimulationClock>().pop_next() {
        Some(e) => e,
        None => return false,
    };
    world.insert_resource(CurrentEvent(event));

    // Track event metrics if EventMetrics resource exists
    if let Some(mut metrics) = world.get_resource_mut::<EventMetrics>() {
        metrics.record_event(event.kind);
    }

    schedule.run(world);
    hook(world, &event);
    true
}

/// Runs simulation steps until the event queue is empty or `max_steps` is reached.
/// Returns the number of steps executed.
pub fn run_until_empty(world: &mut World, schedule: &mut Schedule, max_steps: usize) -> usize {
    let mut steps = 0;
    while steps < max_steps && run_next_event(world, schedule) {
        steps += 1;
    }
    steps
}

/// Runs simulation steps until empty and invokes `hook` after each step.
pub fn run_until_empty_with_hook<F>(
    world: &mut World,
    schedule: &mut Schedule,
    max_steps: usize,
    mut hook: F,
) -> usize
where
    F: FnMut(&World, &Event),
{
    let mut steps = 0;
    while steps < max_steps && run_next_event_with_hook(world, schedule, &mut hook) {
        steps += 1;
    }
    steps
}

/// Builds the default simulation schedule: all event-reacting systems plus [apply_deferred]
/// so that spawned entities (e.g. [crate::ecs::Trip]) are applied before the next step.
///
/// Systems are conditionally executed based on event type to reduce overhead.
pub fn simulation_schedule() -> Schedule {
    let mut schedule = Schedule::default();

    // Group systems by event type using conditions to avoid running all systems on every event
    schedule.add_systems((
        // SimulationStarted
        simulation_started_system.run_if(is_simulation_started),
        // SpawnRider
        rider_spawner_system.run_if(is_spawn_rider),
        // SpawnDriver
        driver_spawner_system.run_if(is_spawn_driver),
        // ShowQuote
        show_quote_system.run_if(is_show_quote),
        // QuoteDecision
        quote_decision_system.run_if(is_quote_decision),
        // QuoteAccepted
        quote_accepted_system.run_if(is_quote_accepted),
        // QuoteRejected
        quote_rejected_system.run_if(is_quote_rejected),
        // TryMatch
        matching_system.run_if(is_try_match),
        // BatchMatchRun
        batch_matching_system.run_if(is_batch_match_run),
        // MatchAccepted
        match_accepted_system.run_if(is_match_accepted),
        // DriverDecision
        driver_decision_system.run_if(is_driver_decision),
        // RiderCancel
        rider_cancel_system.run_if(is_rider_cancel),
        // MoveStep
        movement_system.run_if(is_move_step),
        // PickupEtaUpdated
        pickup_eta_updated_system.run_if(is_pickup_eta_updated),
        // TripStarted
        trip_started_system.run_if(is_trip_started),
        // TripCompleted
        trip_completed_system.run_if(is_trip_completed),
        // CheckDriverOffDuty
        driver_offduty_check_system.run_if(is_check_driver_offduty),
        // Always run apply_deferred to ensure spawned entities are available
        apply_deferred,
    ));

    // Spatial index updates run after apply_deferred so spawned entities are available
    // These run on every event to keep the index in sync
    schedule.add_systems((
        update_spatial_index_riders_system,
        update_spatial_index_drivers_system,
    ));

    // Telemetry snapshot runs conditionally based on interval to avoid overhead
    schedule.add_systems(capture_snapshot_system.run_if(should_capture_snapshot));

    schedule
}

/// Initializes the simulation by scheduling the SimulationStarted event at time 0.
/// Call this after building the scenario and before running events.
pub fn initialize_simulation(world: &mut World) {
    let mut clock = world.resource_mut::<SimulationClock>();
    clock.schedule_at(0, EventKind::SimulationStarted, None);
}
