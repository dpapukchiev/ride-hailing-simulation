//! Simulation runner: advances the clock and routes events into the ECS.
//!
//! Clock progression and event routing happen here, outside systems. Each step
//! pops the next event from [SimulationClock], inserts it as [CurrentEvent],
//! then runs the schedule.

use bevy_ecs::prelude::{Schedule, World};
use bevy_ecs::schedule::apply_deferred;

use crate::clock::{CurrentEvent, SimulationClock};
use crate::systems::{
    driver_decision::driver_decision_system, match_accepted::match_accepted_system,
    movement::movement_system, quote_accepted::quote_accepted_system,
    request_inbound::request_inbound_system, simple_matching::simple_matching_system,
    trip_completed::trip_completed_system, trip_started::trip_started_system,
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

/// Runs simulation steps until the event queue is empty or `max_steps` is reached.
/// Returns the number of steps executed.
pub fn run_until_empty(world: &mut World, schedule: &mut Schedule, max_steps: usize) -> usize {
    let mut steps = 0;
    while steps < max_steps && run_next_event(world, schedule) {
        steps += 1;
    }
    steps
}

/// Builds the default simulation schedule: all event-reacting systems plus [apply_deferred]
/// so that spawned entities (e.g. [crate::ecs::Trip]) are applied before the next step.
pub fn simulation_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems((
        request_inbound_system,
        quote_accepted_system,
        simple_matching_system,
        match_accepted_system,
        driver_decision_system,
        movement_system,
        trip_started_system,
        trip_completed_system,
        apply_deferred,
    ));
    schedule
}
