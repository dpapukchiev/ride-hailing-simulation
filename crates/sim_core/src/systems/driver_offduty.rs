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
