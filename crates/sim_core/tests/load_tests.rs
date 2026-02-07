//! Load tests for sim_core: validate performance under realistic load conditions.

use bevy_ecs::prelude::World;
use sim_core::runner::{initialize_simulation, run_until_empty, simulation_schedule};
use sim_core::scenario::{build_scenario, ScenarioParams};
use std::time::Instant;

#[test]
#[ignore] // Only run explicitly: cargo test --package sim_core --test load_tests -- --ignored
fn test_sustained_load() {
    let mut world = World::new();
    let params = ScenarioParams {
        num_drivers: 500,
        num_riders: 1000,
        ..Default::default()
    }
    .with_seed(42)
    .with_request_window_hours(1)
    .with_driver_spread_hours(1)
    .with_simulation_end_time_ms(60 * 60 * 1000); // 1 hour

    build_scenario(&mut world, params);

    let start = Instant::now();
    initialize_simulation(&mut world);
    let mut schedule = simulation_schedule();
    let events = run_until_empty(&mut world, &mut schedule, 10_000_000);
    let duration = start.elapsed();

    let events_per_sec = events as f64 / duration.as_secs_f64();
    println!(
        "Sustained load test: {} events in {:.2}s ({:.0} events/sec)",
        events,
        duration.as_secs_f64(),
        events_per_sec
    );

    // Assert minimum performance threshold
    assert!(
        events_per_sec > 1000.0,
        "Should process >1000 events/sec, got {:.0}",
        events_per_sec
    );
}

#[test]
#[ignore]
fn test_peak_load() {
    // Test sudden spike: many riders spawn quickly
    let mut world = World::new();
    let params = ScenarioParams {
        num_drivers: 200,
        num_riders: 1000, // High rider count
        ..Default::default()
    }
    .with_seed(42)
    .with_request_window_hours(1) // All riders in 1 hour = spike
    .with_driver_spread_hours(1)
    .with_simulation_end_time_ms(60 * 60 * 1000);

    build_scenario(&mut world, params);

    let start = Instant::now();
    initialize_simulation(&mut world);
    let mut schedule = simulation_schedule();
    let events = run_until_empty(&mut world, &mut schedule, 10_000_000);
    let duration = start.elapsed();

    let events_per_sec = events as f64 / duration.as_secs_f64();
    println!(
        "Peak load test: {} events in {:.2}s ({:.0} events/sec)",
        events,
        duration.as_secs_f64(),
        events_per_sec
    );

    // Should handle peak load gracefully
    assert!(
        events_per_sec > 500.0,
        "Should process >500 events/sec under peak load, got {:.0}",
        events_per_sec
    );
}

#[test]
#[ignore]
fn test_long_running() {
    // Long-running test: 24-hour simulation (or scaled equivalent)
    // This tests for memory leaks and stability over time
    let mut world = World::new();
    let params = ScenarioParams {
        num_drivers: 200,
        num_riders: 500,
        ..Default::default()
    }
    .with_seed(42)
    .with_request_window_hours(24)
    .with_driver_spread_hours(24)
    .with_simulation_end_time_ms(24 * 60 * 60 * 1000); // 24 hours

    build_scenario(&mut world, params);

    let start = Instant::now();
    initialize_simulation(&mut world);
    let mut schedule = simulation_schedule();
    let events = run_until_empty(&mut world, &mut schedule, 50_000_000);
    let duration = start.elapsed();

    let events_per_sec = events as f64 / duration.as_secs_f64();
    println!(
        "Long-running test: {} events in {:.2}s ({:.0} events/sec)",
        events,
        duration.as_secs_f64(),
        events_per_sec
    );

    // Should maintain consistent performance
    assert!(
        events_per_sec > 500.0,
        "Should process >500 events/sec in long-running scenario, got {:.0}",
        events_per_sec
    );
}
