//! Performance benchmarks for sim_core using Criterion.rs.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use bevy_ecs::prelude::World;
use sim_core::runner::{initialize_simulation, run_until_empty, simulation_schedule};
use sim_core::scenario::{build_scenario, ScenarioParams};

fn bench_simulation_run(c: &mut Criterion) {
    let scenarios = vec![
        ("small", 50, 100),
        ("medium", 200, 500),
        ("large", 500, 1000),
    ];
    
    let mut group = c.benchmark_group("simulation_run");
    for (name, drivers, riders) in scenarios {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &(drivers, riders),
            |b, &(drivers, riders)| {
                b.iter(|| {
                    let mut world = World::new();
                    let params = ScenarioParams {
                        num_drivers: drivers,
                        num_riders: riders,
                        ..Default::default()
                    }
                    .with_seed(42)
                    .with_request_window_hours(1)
                    .with_driver_spread_hours(1)
                    .with_simulation_end_time_ms(60 * 60 * 1000); // 1 hour
                    
                    build_scenario(&mut world, params);
                    initialize_simulation(&mut world);
                    let mut schedule = simulation_schedule();
                    black_box(run_until_empty(&mut world, &mut schedule, 1_000_000));
                });
            },
        );
    }
    group.finish();
}

fn bench_matching_algorithms(c: &mut Criterion) {
    use sim_core::matching::{CostBasedMatching, HungarianMatching, SimpleMatching};
    use sim_core::matching::algorithm::MatchingAlgorithm;
    use sim_core::test_helpers::test_cell;
    use bevy_ecs::prelude::Entity;
    
    let rider_pos = test_cell();
    let rider_entity = Entity::from_raw(1);
    
    // Create 100 drivers in nearby cells
    let mut drivers = Vec::new();
    let disk = rider_pos.grid_disk::<Vec<_>>(5);
    for (i, cell) in disk.iter().take(100).enumerate() {
        drivers.push((Entity::from_raw(i as u32 + 2), *cell));
    }
    
    let mut group = c.benchmark_group("matching_algorithms");
    
    // Simple matching
    let simple = SimpleMatching::default();
    group.bench_function("simple_100_drivers", |b| {
        b.iter(|| {
            black_box(simple.find_match(
                rider_entity,
                rider_pos,
                None,
                &drivers,
                5,
                0,
            ));
        });
    });
    
    // Cost-based matching
    let cost_based = CostBasedMatching::default();
    group.bench_function("cost_based_100_drivers", |b| {
        b.iter(|| {
            black_box(cost_based.find_match(
                rider_entity,
                rider_pos,
                None,
                &drivers,
                5,
                0,
            ));
        });
    });
    
    // Hungarian batch matching
    let hungarian = HungarianMatching::default();
    let riders = vec![(rider_entity, rider_pos, None)];
    group.bench_function("hungarian_100x200", |b| {
        let mut drivers_200 = drivers.clone();
        // Extend to 200 drivers
        let disk2 = rider_pos.grid_disk::<Vec<_>>(10);
        for (i, cell) in disk2.iter().skip(100).take(100).enumerate() {
            drivers_200.push((Entity::from_raw(i as u32 + 102), *cell));
        }
        b.iter(|| {
            black_box(hungarian.find_batch_matches(
                &riders,
                &drivers_200,
                5,
                0,
            ));
        });
    });
    
    group.finish();
}

criterion_group!(benches, bench_simulation_run, bench_matching_algorithms);
criterion_main!(benches);
