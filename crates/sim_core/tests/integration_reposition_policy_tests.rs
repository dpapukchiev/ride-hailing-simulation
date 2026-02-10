use bevy_ecs::prelude::World;
use sim_core::runner::{initialize_simulation, run_until_empty, simulation_schedule};
use sim_core::scenario::{build_scenario, RepositionPolicyConfig, ScenarioParams};
use sim_core::telemetry::SimTelemetry;

fn run_pickup_metrics(params: ScenarioParams) -> (f64, u64, f64) {
    let mut world = World::new();
    build_scenario(&mut world, params);
    let mut schedule = simulation_schedule();
    initialize_simulation(&mut world);
    let _ = run_until_empty(&mut world, &mut schedule, 20_000);

    let telemetry = world.resource::<SimTelemetry>();
    let mut pickups: Vec<u64> = telemetry
        .completed_trips
        .iter()
        .map(|trip| trip.time_to_pickup())
        .collect();
    if pickups.is_empty() {
        return (0.0, 0, 0.0);
    }
    pickups.sort_unstable();
    let mean = pickups.iter().sum::<u64>() as f64 / pickups.len() as f64;
    let p95 = pickups[((pickups.len() - 1) as f64 * 0.95) as usize];
    let within_5m =
        pickups.iter().filter(|eta| **eta <= 300_000).count() as f64 / pickups.len() as f64;
    (mean, p95, within_5m)
}

#[test]
fn hotspot_spike_and_shift_and_sparse_regime_have_nonzero_coverage() {
    let policy = Some(RepositionPolicyConfig::default());

    let demand_spike = ScenarioParams {
        num_riders: 200,
        num_drivers: 90,
        request_window_ms: 40 * 60 * 1000,
        spawn_weighting: sim_core::spawner::SpawnWeightingKind::BerlinHotspots,
        seed: Some(7),
        reposition_policy_config: policy,
        ..Default::default()
    };
    let shifting_hotspot = ScenarioParams {
        num_riders: 220,
        num_drivers: 95,
        request_window_ms: 80 * 60 * 1000,
        traffic_profile: sim_core::traffic::TrafficProfileKind::Berlin,
        seed: Some(11),
        reposition_policy_config: policy,
        ..Default::default()
    };
    let sparse_supply = ScenarioParams {
        num_riders: 240,
        num_drivers: 55,
        request_window_ms: 60 * 60 * 1000,
        seed: Some(99),
        reposition_policy_config: policy,
        ..Default::default()
    };

    let (mean_a, p95_a, near_a) = run_pickup_metrics(demand_spike.clone());
    let (mean_b, p95_b, near_b) = run_pickup_metrics(shifting_hotspot.clone());
    let (mean_c, p95_c, near_c) = run_pickup_metrics(sparse_supply.clone());

    let repeat_a = run_pickup_metrics(demand_spike);
    let repeat_b = run_pickup_metrics(shifting_hotspot);
    let repeat_c = run_pickup_metrics(sparse_supply);

    println!("demand_spike: mean_ms={mean_a:.1}, p95_ms={p95_a}, within5m={near_a:.3}");
    println!("shifting_hotspot: mean_ms={mean_b:.1}, p95_ms={p95_b}, within5m={near_b:.3}");
    println!("sparse_supply: mean_ms={mean_c:.1}, p95_ms={p95_c}, within5m={near_c:.3}");
    assert_eq!((mean_a, p95_a, near_a), repeat_a);
    assert_eq!((mean_b, p95_b, near_b), repeat_b);
    assert_eq!((mean_c, p95_c, near_c), repeat_c);
    assert!((0.0..=1.0).contains(&near_a));
    assert!((0.0..=1.0).contains(&near_b));
    assert!((0.0..=1.0).contains(&near_c));
}
