mod support;

use bevy_ecs::prelude::World;
use sim_core::scenario::{build_scenario, MatchRadius, ScenarioParams};
use sim_core::spawner::{DriverSpawner, RiderSpawner};

#[test]
fn build_scenario_configures_spawners() {
    let mut world = World::new();
    build_scenario(
        &mut world,
        ScenarioParams {
            num_riders: 10,
            num_drivers: 3,
            seed: Some(42),
            ..Default::default()
        },
    );

    let rider_spawner = world.resource::<RiderSpawner>();
    assert_eq!(rider_spawner.config.max_count, Some(10));
    assert_eq!(rider_spawner.spawned_count(), 0);

    let driver_spawner = world.resource::<DriverSpawner>();
    assert_eq!(driver_spawner.config.max_count, Some(3));
    assert_eq!(driver_spawner.spawned_count(), 0);
}

#[test]
fn build_scenario_handles_empty_scenarios() {
    let mut world = World::new();
    build_scenario(
        &mut world,
        ScenarioParams {
            num_riders: 0,
            num_drivers: 0,
            seed: Some(42),
            ..Default::default()
        },
    );

    let rider_spawner = world.resource::<RiderSpawner>();
    assert_eq!(rider_spawner.config.max_count, Some(0));

    let driver_spawner = world.resource::<DriverSpawner>();
    assert_eq!(driver_spawner.config.max_count, Some(0));
}

#[test]
fn build_scenario_handles_zero_match_radius() {
    let mut world = World::new();
    build_scenario(
        &mut world,
        ScenarioParams {
            num_riders: 5,
            num_drivers: 2,
            match_radius: 0,
            seed: Some(42),
            ..Default::default()
        },
    );

    let match_radius = world.resource::<MatchRadius>();
    assert_eq!(match_radius.0, 0);
}

#[test]
fn build_scenario_handles_initial_counts() {
    let mut world = World::new();
    build_scenario(
        &mut world,
        ScenarioParams {
            num_riders: 10,
            num_drivers: 5,
            initial_rider_count: 3,
            initial_driver_count: 2,
            seed: Some(42),
            ..Default::default()
        },
    );

    let rider_spawner = world.resource::<RiderSpawner>();
    assert_eq!(rider_spawner.config.max_count, Some(7));
    assert_eq!(rider_spawner.config.initial_count, 3);

    let driver_spawner = world.resource::<DriverSpawner>();
    assert_eq!(driver_spawner.config.max_count, Some(3));
    assert_eq!(driver_spawner.config.initial_count, 2);
}
