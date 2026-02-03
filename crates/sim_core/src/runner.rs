//! Simulation runner: advances the clock and routes events into the ECS.
//!
//! Clock progression and event routing happen here, outside systems. Each step
//! pops the next event from [SimulationClock], inserts it as [CurrentEvent],
//! then runs the schedule.

use bevy_ecs::prelude::{Schedule, World};
use bevy_ecs::schedule::apply_deferred;

use crate::clock::{CurrentEvent, Event, EventKind, SimulationClock};
use crate::systems::{
    driver_decision::driver_decision_system, driver_offduty::driver_offduty_check_system,
    match_accepted::match_accepted_system,
    movement::movement_system, pickup_eta_updated::pickup_eta_updated_system,
    quote_accepted::quote_accepted_system, quote_decision::quote_decision_system,
    quote_rejected::quote_rejected_system, show_quote::show_quote_system,
    rider_cancel::rider_cancel_system,
    spawner::{driver_spawner_system, rider_spawner_system, simulation_started_system},
    matching::matching_system,
    telemetry_snapshot::capture_snapshot_system, trip_completed::trip_completed_system,
    trip_started::trip_started_system,
};

/// Runs one simulation step: pops the next event, inserts it as [CurrentEvent], then runs the schedule.
/// Returns `true` if an event was processed, `false` if the clock was empty.
pub fn run_next_event(world: &mut World, schedule: &mut Schedule) -> bool {
    let event = match world.resource_mut::<SimulationClock>().pop_next() {
        Some(e) => e,
        None => return false,
    };
    world.insert_resource(CurrentEvent(event));
    schedule.run(world);
    true
}

/// Runs one simulation step and invokes `hook` after the schedule completes.
pub fn run_next_event_with_hook<F>(world: &mut World, schedule: &mut Schedule, mut hook: F) -> bool
where
    F: FnMut(&World, &Event),
{
    let event = match world.resource_mut::<SimulationClock>().pop_next() {
        Some(e) => e,
        None => return false,
    };
    world.insert_resource(CurrentEvent(event));
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
pub fn simulation_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems((
        simulation_started_system,
        rider_spawner_system,
        driver_spawner_system,
        show_quote_system,
        quote_decision_system,
        quote_accepted_system,
        quote_rejected_system,
        matching_system,
        match_accepted_system,
        driver_decision_system,
        rider_cancel_system,
        movement_system,
        pickup_eta_updated_system,
        trip_started_system,
        trip_completed_system,
        driver_offduty_check_system,
        apply_deferred,
    ));
    schedule.add_systems(capture_snapshot_system);
    schedule
}

/// Initializes the simulation by scheduling the SimulationStarted event at time 0.
/// Call this after building the scenario and before running events.
pub fn initialize_simulation(world: &mut World) {
    let mut clock = world.resource_mut::<SimulationClock>();
    clock.schedule_at(0, EventKind::SimulationStarted, None);
}
