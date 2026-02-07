//! Large-scale scenario: 10,000 riders / 7,000 drivers.
//!
//! Demonstrates simulation performance at scale and reports throughput metrics.
//!
//! Run with: cargo run -p sim_core --example scenario_run_large --release
//! Export:   SIM_EXPORT_DIR=/path cargo run -p sim_core --example scenario_run_large --release

use bevy_ecs::prelude::World;
use sim_core::pricing::PricingConfig;
use sim_core::profiling::EventMetrics;
use sim_core::runner::{run_until_empty, simulation_schedule};
use sim_core::scenario::{build_scenario, ScenarioParams};
use sim_core::telemetry_export::{
    write_agent_positions_parquet, write_completed_trips_parquet, write_snapshot_counts_parquet,
    write_trips_parquet,
};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    const NUM_RIDERS: usize = 10_000;
    const NUM_DRIVERS: usize = 7_000;
    const SIMULATION_HOURS: u64 = 4;

    println!(
        "=== Large-Scale Scenario ({} riders, {} drivers, {}h) ===\n",
        NUM_RIDERS, NUM_DRIVERS, SIMULATION_HOURS
    );

    // --- Build phase ---
    let build_start = Instant::now();
    let mut world = World::new();
    world.insert_resource(EventMetrics::default());
    build_scenario(
        &mut world,
        ScenarioParams {
            num_riders: NUM_RIDERS,
            num_drivers: NUM_DRIVERS,
            ..Default::default()
        }
        .with_seed(42)
        .with_request_window_hours(SIMULATION_HOURS)
        .with_match_radius(5)
        .with_trip_duration_cells(5, 60)
        .with_simulation_end_time_ms(SIMULATION_HOURS * 60 * 60 * 1000)
        .with_pricing_config(PricingConfig {
            base_fare: 2.50,
            per_km_rate: 1.50,
            commission_rate: 0.15,
            surge_enabled: true,
            surge_radius_k: 2,
            surge_max_multiplier: 1.3,
        }),
    );
    sim_core::runner::initialize_simulation(&mut world);
    let build_elapsed = build_start.elapsed();
    println!("Build time: {:.2}s", build_elapsed.as_secs_f64());

    // --- Run phase ---
    let run_start = Instant::now();
    let mut schedule = simulation_schedule();
    let max_steps = 20_000_000;
    let steps = run_until_empty(&mut world, &mut schedule, max_steps);
    let run_elapsed = run_start.elapsed();

    // --- Collect metrics ---
    let telemetry = world.resource::<sim_core::telemetry::SimTelemetry>();
    let clock = world.resource::<sim_core::clock::SimulationClock>();
    let sim_time_secs = clock.now() / 1000;
    let completed = telemetry.completed_trips.len();

    println!("\n--- Simulation Results ---");
    println!("Steps executed:      {}", steps);
    println!(
        "Simulation time:     {} s ({:.1} min)",
        sim_time_secs,
        sim_time_secs as f64 / 60.0
    );
    println!("Wall-clock time:     {:.2}s", run_elapsed.as_secs_f64());
    println!(
        "Events per second:   {:.0}",
        steps as f64 / run_elapsed.as_secs_f64()
    );

    println!("\n--- Outcomes ---");
    println!("Completed trips:     {}", completed);
    println!(
        "Cancelled (pickup):  {}",
        telemetry.riders_cancelled_pickup_timeout
    );
    println!(
        "Abandoned (quote):   {}",
        telemetry.riders_abandoned_quote_total
    );
    println!("  - price too high:  {}", telemetry.riders_abandoned_price);
    println!("  - ETA too long:    {}", telemetry.riders_abandoned_eta);
    println!(
        "  - stochastic:      {}",
        telemetry.riders_abandoned_stochastic
    );
    println!(
        "Platform revenue:    ${:.2}",
        telemetry.platform_revenue_total
    );
    println!(
        "Total fares:         ${:.2}",
        telemetry.total_fares_collected
    );

    if completed > 0 {
        let mut match_times: Vec<u64> = telemetry
            .completed_trips
            .iter()
            .map(|r| r.time_to_match())
            .collect();
        let mut pickup_times: Vec<u64> = telemetry
            .completed_trips
            .iter()
            .map(|r| r.time_to_pickup())
            .collect();
        let mut trip_durations: Vec<u64> = telemetry
            .completed_trips
            .iter()
            .map(|r| r.trip_duration())
            .collect();
        match_times.sort_unstable();
        pickup_times.sort_unstable();
        trip_durations.sort_unstable();

        let p50 = |v: &[u64]| v[v.len() / 2];
        let p90 = |v: &[u64]| v[(v.len() as f64 * 0.9) as usize];
        let p99 = |v: &[u64]| v[(v.len() as f64 * 0.99) as usize];
        let avg = |v: &[u64]| v.iter().sum::<u64>() as f64 / v.len() as f64;

        println!("\n--- Timing Distributions (seconds) ---");
        println!(
            "{:20} {:>8} {:>8} {:>8} {:>8} {:>8}",
            "", "avg", "p50", "p90", "p99", "max"
        );
        println!(
            "{:20} {:>8.1} {:>8.1} {:>8.1} {:>8.1} {:>8.1}",
            "Time to match",
            avg(&match_times) / 1000.0,
            p50(&match_times) as f64 / 1000.0,
            p90(&match_times) as f64 / 1000.0,
            p99(&match_times) as f64 / 1000.0,
            *match_times.last().unwrap_or(&0) as f64 / 1000.0,
        );
        println!(
            "{:20} {:>8.1} {:>8.1} {:>8.1} {:>8.1} {:>8.1}",
            "Time to pickup",
            avg(&pickup_times) / 1000.0,
            p50(&pickup_times) as f64 / 1000.0,
            p90(&pickup_times) as f64 / 1000.0,
            p99(&pickup_times) as f64 / 1000.0,
            *pickup_times.last().unwrap_or(&0) as f64 / 1000.0,
        );
        println!(
            "{:20} {:>8.1} {:>8.1} {:>8.1} {:>8.1} {:>8.1}",
            "Trip duration",
            avg(&trip_durations) / 1000.0,
            p50(&trip_durations) as f64 / 1000.0,
            p90(&trip_durations) as f64 / 1000.0,
            p99(&trip_durations) as f64 / 1000.0,
            *trip_durations.last().unwrap_or(&0) as f64 / 1000.0,
        );
    }

    // --- Event breakdown ---
    let event_metrics = world.resource::<EventMetrics>();
    event_metrics.print_summary();

    // --- Export ---
    if let Ok(export_dir) = env::var("SIM_EXPORT_DIR") {
        let export_path = PathBuf::from(export_dir);
        if let Err(err) = fs::create_dir_all(&export_path) {
            eprintln!("Failed to create export dir {:?}: {}", export_path, err);
            return;
        }
        let trips_path = export_path.join("completed_trips.parquet");
        let counts_path = export_path.join("snapshot_counts.parquet");
        let positions_path = export_path.join("agent_positions.parquet");
        let all_trips_path = export_path.join("trips.parquet");

        let snapshots = world.resource::<sim_core::telemetry::SimSnapshots>();
        if let Err(err) = write_completed_trips_parquet(&trips_path, telemetry) {
            eprintln!("Failed to export completed trips: {}", err);
        }
        if let Err(err) = write_snapshot_counts_parquet(&counts_path, snapshots) {
            eprintln!("Failed to export snapshot counts: {}", err);
        }
        if let Err(err) = write_agent_positions_parquet(&positions_path, snapshots) {
            eprintln!("Failed to export agent positions: {}", err);
        }
        if let Err(err) = write_trips_parquet(&all_trips_path, snapshots) {
            eprintln!("Failed to export trips: {}", err);
        }

        println!("\nExported Parquet files to {:?}", export_path);
    }

    println!("\n=== Done ===");
}
