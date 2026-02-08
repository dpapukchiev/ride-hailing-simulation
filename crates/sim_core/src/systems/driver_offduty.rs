//! System for checking if drivers should go OffDuty based on earnings targets and fatigue thresholds.

use bevy_ecs::prelude::{Commands, Entity, Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock, ONE_MIN_MS};
use crate::ecs::{Driver, DriverEarnings, DriverFatigue, DriverStateCommands, OffDuty};

/// Interval for periodic OffDuty checks (5 minutes).
const CHECK_INTERVAL_MS: u64 = 5 * ONE_MIN_MS;

/// Check a single driver for earnings/fatigue thresholds.
/// Transitions the driver to OffDuty and sets session_end_time_ms if thresholds are exceeded.
fn check_driver_offduty(
    commands: &mut Commands,
    now: u64,
    driver_entity: Entity,
    _driver: &mut Driver,
    earnings: &mut DriverEarnings,
    fatigue: &DriverFatigue,
    is_offduty: bool,
) {
    if is_offduty {
        return;
    }

    let mut should_go_offduty = false;

    if earnings.daily_earnings >= earnings.daily_earnings_target {
        should_go_offduty = true;
    }

    let session_duration_ms = now.saturating_sub(earnings.session_start_time_ms);
    if session_duration_ms >= fatigue.fatigue_threshold_ms {
        should_go_offduty = true;
    }

    if should_go_offduty {
        earnings.session_end_time_ms = Some(now);
        commands.entity(driver_entity).set_driver_state_off_duty();
    }
}

/// System that checks if drivers should go OffDuty based on earnings targets and fatigue thresholds.
///
/// Supports two modes:
/// - **Periodic** (no subject): iterates all drivers, then schedules the next periodic check.
/// - **Targeted** (`EventSubject::Driver(entity)`): checks only the specified driver. Used by
///   `trip_completed_system` to give immediate feedback after earnings are updated.
///
/// Also bootstraps the periodic check cycle on `SimulationStarted`.
pub fn driver_offduty_check_system(
    mut commands: Commands,
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    mut drivers: Query<(
        Entity,
        &mut Driver,
        &mut DriverEarnings,
        &DriverFatigue,
        Option<&OffDuty>,
    )>,
) {
    if event.0.kind == EventKind::CheckDriverOffDuty {
        let now = clock.now();

        match event.0.subject {
            // Targeted check: only the specified driver (e.g. after trip completion)
            Some(EventSubject::Driver(driver_entity)) => {
                if let Ok((entity, mut driver, mut earnings, fatigue, offduty)) =
                    drivers.get_mut(driver_entity)
                {
                    check_driver_offduty(
                        &mut commands,
                        now,
                        entity,
                        &mut driver,
                        &mut earnings,
                        fatigue,
                        offduty.is_some(),
                    );
                }
            }
            // Periodic check: iterate all drivers, then schedule next check
            _ => {
                for (entity, mut driver, mut earnings, fatigue, offduty) in drivers.iter_mut() {
                    check_driver_offduty(
                        &mut commands,
                        now,
                        entity,
                        &mut driver,
                        &mut earnings,
                        fatigue,
                        offduty.is_some(),
                    );
                }
                clock.schedule_in(CHECK_INTERVAL_MS, EventKind::CheckDriverOffDuty, None);
            }
        }
        return;
    }

    // Bootstrap periodic checks on simulation start
    if event.0.kind == EventKind::SimulationStarted {
        clock.schedule_in(CHECK_INTERVAL_MS, EventKind::CheckDriverOffDuty, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::SimulationClock;
    use crate::ecs::{EnRoute, Idle};
    use bevy_ecs::prelude::{Schedule, World};
    use bevy_ecs::schedule::apply_deferred;

    #[test]
    fn driver_goes_offduty_when_earnings_target_reached() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());

        let driver_entity = world
            .spawn((
                Driver {
                    matched_rider: None,
                    assigned_trip: None,
                },
                Idle,
                DriverEarnings {
                    daily_earnings: 150.0,
                    daily_earnings_target: 100.0,
                    session_start_time_ms: 0,
                    session_end_time_ms: None,
                },
                DriverFatigue {
                    fatigue_threshold_ms: 8 * 60 * 60 * 1000, // 8 hours
                },
            ))
            .id();

        world
            .resource_mut::<SimulationClock>()
            .schedule_at(0, EventKind::SimulationStarted, None);
        world.resource_mut::<SimulationClock>().schedule_in(
            5 * ONE_MIN_MS,
            EventKind::CheckDriverOffDuty,
            None,
        );

        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("simulation started event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems((driver_offduty_check_system, apply_deferred));
        schedule.run(&mut world);

        // Process the check event
        let check_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("check driver offduty event");
        world.insert_resource(CurrentEvent(check_event));
        schedule.run(&mut world);

        let is_offduty = world.entity(driver_entity).contains::<OffDuty>();
        assert!(is_offduty);
    }

    #[test]
    fn driver_goes_offduty_when_fatigue_threshold_exceeded() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());

        let target_time = 9 * 60 * 60 * 1000; // 9 hours
        let driver_entity = world
            .spawn((
                Driver {
                    matched_rider: None,
                    assigned_trip: None,
                },
                Idle,
                DriverEarnings {
                    daily_earnings: 50.0,
                    daily_earnings_target: 200.0,
                    session_start_time_ms: 0, // Started at time 0
                    session_end_time_ms: None,
                },
                DriverFatigue {
                    fatigue_threshold_ms: 8 * 60 * 60 * 1000, // 8 hours
                },
            ))
            .id();

        // Schedule the check event at target time
        world.resource_mut::<SimulationClock>().schedule_at(
            target_time,
            EventKind::CheckDriverOffDuty,
            None,
        );

        // Pop the event - this will advance clock.now() to target_time
        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("check driver offduty event");

        // Verify clock advanced
        assert_eq!(world.resource::<SimulationClock>().now(), target_time);

        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems((driver_offduty_check_system, apply_deferred));
        schedule.run(&mut world);

        let is_offduty = world.entity(driver_entity).contains::<OffDuty>();
        assert!(is_offduty);
    }

    #[test]
    fn driver_goes_offduty_when_fatigue_exceeded_while_en_route() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());

        let target_time = 9 * 60 * 60 * 1000; // 9 hours
        let driver_entity = world
            .spawn((
                Driver {
                    matched_rider: None,
                    assigned_trip: None,
                },
                EnRoute,
                DriverEarnings {
                    daily_earnings: 50.0,
                    daily_earnings_target: 200.0,
                    session_start_time_ms: 0,
                    session_end_time_ms: None,
                },
                DriverFatigue {
                    fatigue_threshold_ms: 8 * 60 * 60 * 1000, // 8 hours
                },
            ))
            .id();

        world.resource_mut::<SimulationClock>().schedule_at(
            target_time,
            EventKind::CheckDriverOffDuty,
            None,
        );

        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("check driver offduty event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems((driver_offduty_check_system, apply_deferred));
        schedule.run(&mut world);

        let is_offduty = world.entity(driver_entity).contains::<OffDuty>();
        assert!(
            is_offduty,
            "driver over fatigue threshold should go OffDuty even when EnRoute"
        );
    }
}
