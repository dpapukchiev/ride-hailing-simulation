mod support;

use bevy_ecs::prelude::{Entity, Schedule, World};
use bevy_ecs::schedule::apply_deferred;
use sim_core::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock, ONE_MIN_MS};
use sim_core::ecs::{Driver, DriverEarnings, DriverFatigue, EnRoute, Idle, OffDuty};
use sim_core::systems::driver_offduty::driver_offduty_check_system;

fn spawn_driver(
    world: &mut World,
    earnings: f64,
    earnings_target: f64,
    fatigue_threshold_ms: u64,
    en_route: bool,
) -> Entity {
    if en_route {
        world
            .spawn((
                Driver {
                    matched_rider: None,
                    assigned_trip: None,
                },
                EnRoute,
                DriverEarnings {
                    daily_earnings: earnings,
                    daily_earnings_target: earnings_target,
                    session_start_time_ms: 0,
                    session_end_time_ms: None,
                },
                DriverFatigue {
                    fatigue_threshold_ms,
                },
            ))
            .id()
    } else {
        world
            .spawn((
                Driver {
                    matched_rider: None,
                    assigned_trip: None,
                },
                Idle,
                DriverEarnings {
                    daily_earnings: earnings,
                    daily_earnings_target: earnings_target,
                    session_start_time_ms: 0,
                    session_end_time_ms: None,
                },
                DriverFatigue {
                    fatigue_threshold_ms,
                },
            ))
            .id()
    }
}

fn run_for_event(world: &mut World, event: sim_core::clock::Event) {
    world.insert_resource(CurrentEvent(event));
    let mut schedule = Schedule::default();
    schedule.add_systems((driver_offduty_check_system, apply_deferred));
    schedule.run(world);
}

#[test]
fn driver_goes_offduty_when_earnings_target_reached() {
    let mut world = World::new();
    world.insert_resource(SimulationClock::default());
    let driver_entity = spawn_driver(&mut world, 150.0, 100.0, 8 * 60 * 60 * 1000, false);

    world
        .resource_mut::<SimulationClock>()
        .schedule_at(0, EventKind::SimulationStarted, None);
    world.resource_mut::<SimulationClock>().schedule_in(
        5 * ONE_MIN_MS,
        EventKind::CheckDriverOffDuty,
        None,
    );

    let started = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("simulation started event");
    run_for_event(&mut world, started);

    let check = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("check driver offduty event");
    run_for_event(&mut world, check);

    assert!(world.entity(driver_entity).contains::<OffDuty>());
}

#[test]
fn driver_goes_offduty_when_fatigue_threshold_exceeded() {
    let mut world = World::new();
    world.insert_resource(SimulationClock::default());

    let target_time = 9 * 60 * 60 * 1000;
    let driver_entity = spawn_driver(&mut world, 50.0, 200.0, 8 * 60 * 60 * 1000, false);

    world.resource_mut::<SimulationClock>().schedule_at(
        target_time,
        EventKind::CheckDriverOffDuty,
        Some(EventSubject::Driver(driver_entity)),
    );

    let event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("check driver offduty event");

    assert_eq!(world.resource::<SimulationClock>().now(), target_time);
    run_for_event(&mut world, event);

    assert!(world.entity(driver_entity).contains::<OffDuty>());
}

#[test]
fn driver_goes_offduty_when_fatigue_exceeded_while_en_route() {
    let mut world = World::new();
    world.insert_resource(SimulationClock::default());

    let target_time = 9 * 60 * 60 * 1000;
    let driver_entity = spawn_driver(&mut world, 50.0, 200.0, 8 * 60 * 60 * 1000, true);

    world.resource_mut::<SimulationClock>().schedule_at(
        target_time,
        EventKind::CheckDriverOffDuty,
        Some(EventSubject::Driver(driver_entity)),
    );

    let event = world
        .resource_mut::<SimulationClock>()
        .pop_next()
        .expect("check driver offduty event");
    run_for_event(&mut world, event);

    assert!(
        world.entity(driver_entity).contains::<OffDuty>(),
        "driver over fatigue threshold should go OffDuty even when EnRoute"
    );
}
