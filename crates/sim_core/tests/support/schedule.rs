#![allow(dead_code)]

use bevy_ecs::prelude::World;
use bevy_ecs::schedule::Schedule;
use sim_core::runner::{run_next_event, run_until_empty, simulation_schedule};

/// Helper that owns a reusable `Schedule` so tests can step or drain the event queue.
pub struct ScheduleRunner {
    schedule: Schedule,
}

impl Default for ScheduleRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl ScheduleRunner {
    /// Create a runner with the default simulation schedule.
    pub fn new() -> Self {
        Self {
            schedule: simulation_schedule(),
        }
    }

    /// Run a single event (returns `true` if an event was processed).
    pub fn run_one(&mut self, world: &mut World) -> bool {
        run_next_event(world, &mut self.schedule)
    }

    /// Run multiple events up to `max_steps`, returning the number of steps executed.
    pub fn run_until_empty(&mut self, world: &mut World, max_steps: usize) -> usize {
        run_until_empty(world, &mut self.schedule, max_steps)
    }

    /// Drive the simulation until the event queue is empty (or an upper limit is hit).
    pub fn run_full(&mut self, world: &mut World) -> usize {
        self.run_until_empty(world, usize::MAX)
    }
}
