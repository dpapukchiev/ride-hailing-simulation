mod support;

use sim_core::ecs::{Driver, Position, Rider};
use sim_core::runner::initialize_simulation;
use sim_core::scenario::{build_scenario, ScenarioParams};
use sim_core::spawner::{DriverSpawner, RiderSpawner};
use support::schedule::ScheduleRunner;
use support::world::TestWorldBuilder;

fn in_bounds(cell: h3o::CellIndex, lat_min: f64, lat_max: f64, lng_min: f64, lng_max: f64) -> bool {
    let coord: h3o::LatLng = cell.into();
    coord.lat() >= lat_min
        && coord.lat() <= lat_max
        && coord.lng() >= lng_min
        && coord.lng() <= lng_max
}

#[test]
fn spawner_generates_entities_within_configured_bounds() {
    let mut world = TestWorldBuilder::default().with_seed(42).build();

    let params = ScenarioParams {
        num_riders: 4,
        num_drivers: 3,
        initial_rider_count: 1,
        initial_driver_count: 1,
        lat_min: 52.50,
        lat_max: 52.53,
        lng_min: 13.38,
        lng_max: 13.42,
        request_window_ms: 10_000,
        driver_spread_ms: 10_000,
        simulation_end_time_ms: Some(20_000),
        seed: Some(42),
        ..Default::default()
    };

    build_scenario(&mut world, params.clone());
    initialize_simulation(&mut world);

    let mut runner = ScheduleRunner::new();
    let steps = runner.run_until_empty(&mut world, 20_000);
    assert!(steps < 20_000, "runner did not converge");

    let rider_spawner = world.resource::<RiderSpawner>();
    assert!(rider_spawner.spawned_count() > 0);
    assert!(rider_spawner.spawned_count() <= 4);

    let rider_positions: Vec<_> = world
        .query::<(&Rider, &Position)>()
        .iter(&world)
        .map(|(_, position)| position.0)
        .collect();
    assert!(rider_positions.iter().all(|cell| {
        in_bounds(
            *cell,
            params.lat_min,
            params.lat_max,
            params.lng_min,
            params.lng_max,
        )
    }));

    let driver_spawner = world.resource::<DriverSpawner>();
    assert!(driver_spawner.spawned_count() > 0);
    assert!(driver_spawner.spawned_count() <= 3);

    let driver_positions: Vec<_> = world
        .query::<(&Driver, &Position)>()
        .iter(&world)
        .map(|(_, position)| position.0)
        .collect();
    assert!(driver_positions.iter().all(|cell| {
        in_bounds(
            *cell,
            params.lat_min,
            params.lat_max,
            params.lng_min,
            params.lng_max,
        )
    }));
}
