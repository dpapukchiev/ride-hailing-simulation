//! Run the 500 riders / 100 drivers scenario and print completed trips.
//!
//! Run with: cargo run -p sim_core --example scenario_run

use bevy_ecs::prelude::World;
use sim_core::runner::{run_until_empty, simulation_schedule};
use sim_core::scenario::{build_scenario, ScenarioParams};

fn main() {
    const NUM_RIDERS: usize = 500;
    const NUM_DRIVERS: usize = 100;
    const SIMULATION_HOURS: u64 = 4;

    let mut world = World::new();
    build_scenario(
        &mut world,
        ScenarioParams {
            num_riders: NUM_RIDERS,
            num_drivers: NUM_DRIVERS,
            ..Default::default()
        }
        .with_seed(123)
        .with_request_window_hours(SIMULATION_HOURS)
        .with_match_radius(5)
        .with_trip_duration_cells(5, 60),
    );

    let mut schedule = simulation_schedule();
    // 4h of sim time + 500 riders × many events each; allow enough steps to drain the queue
    let max_steps = 2_000_000;
    let steps = run_until_empty(&mut world, &mut schedule, max_steps);

    let telemetry = world.resource::<sim_core::telemetry::SimTelemetry>();
    let completed = telemetry.completed_trips.len();
    let clock = world.resource::<sim_core::clock::SimulationClock>();
    let sim_time_secs = clock.now() / 1000;

    println!("--- Scenario run ({} riders, {} drivers, {}h request window, seed 123) ---", NUM_RIDERS, NUM_DRIVERS, SIMULATION_HOURS);
    println!("Steps executed: {}", steps);
    println!("Simulation time: {} s ({:.1} min)", sim_time_secs, sim_time_secs as f64 / 60.0);
    println!("Completed trips: {}", completed);

    if completed > 0 {
        println!("\nSample completed trips (first 100):");
        const ONE_SEC_MS: u64 = 1000;
        const SAMPLE: usize = 100;
        for (i, r) in telemetry.completed_trips.iter().take(SAMPLE).enumerate() {
            println!(
                "  {}  rider={:?} driver={:?}  time_to_match={} s  time_to_pickup={} s  trip_duration={} s  completed_at={} s",
                i + 1,
                r.rider_entity,
                r.driver_entity,
                r.time_to_match() / ONE_SEC_MS,
                r.time_to_pickup() / ONE_SEC_MS,
                r.trip_duration() / ONE_SEC_MS,
                r.completed_at / ONE_SEC_MS,
            );
        }
        if completed > SAMPLE {
            println!("  ... and {} more", completed - SAMPLE);
        }
    } else {
        println!("\nNo trips completed. (Riders and drivers are randomly placed; many riders may have no idle driver in the same H3 cell.)");
    }
}
