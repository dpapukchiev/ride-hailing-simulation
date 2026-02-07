//! Batch matching system: run a global matching pass when BatchMatchRun fires.
//!
//! Collects all riders in Waiting state and all Idle drivers, calls the matching
//! algorithm's find_batch_matches, applies matches, and schedules the next batch run.

use bevy_ecs::prelude::{Entity, Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Driver, DriverState, Position, Rider, RiderState};
use crate::matching::MatchingAlgorithmResource;
use crate::scenario::{BatchMatchingConfig, MatchRadius};

pub fn batch_matching_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    batch_config: Option<Res<BatchMatchingConfig>>,
    match_radius: Option<Res<MatchRadius>>,
    matching_algorithm: Res<MatchingAlgorithmResource>,
    mut riders: Query<(Entity, &mut Rider, &Position)>,
    mut drivers: Query<(Entity, &mut Driver, &Position)>,
) {
    if event.0.kind != EventKind::BatchMatchRun {
        return;
    }

    let Some(config) = batch_config.as_deref() else {
        return;
    };
    if !config.enabled {
        return;
    }

    let radius = match_radius.as_deref().map(|r| r.0).unwrap_or(0);

    // Collect only riders who are Waiting and not yet assigned (looking for a match).
    // Riders who are Waiting but already have matched_driver are waiting for that driver
    // to accept/drive to pickup and must not be re-matched.
    let waiting_riders: Vec<(Entity, h3o::CellIndex, Option<h3o::CellIndex>)> = riders
        .iter()
        .filter(|(_, rider, _)| {
            rider.state == RiderState::Waiting && rider.matched_driver.is_none()
        })
        .map(|(entity, rider, position)| (entity, position.0, rider.destination))
        .collect();

    // Collect all Idle drivers (exclude OffDuty and others)
    let available_drivers: Vec<(Entity, h3o::CellIndex)> = drivers
        .iter()
        .filter_map(|(entity, driver, position)| {
            if driver.state == DriverState::Idle {
                Some((entity, position.0))
            } else {
                None
            }
        })
        .collect();

    let matches = matching_algorithm.find_batch_matches(
        &waiting_riders,
        &available_drivers,
        radius,
        clock.now(),
    );

    for m in matches {
        if let Ok((_, mut rider, _)) = riders.get_mut(m.rider_entity) {
            rider.matched_driver = Some(m.driver_entity);
        }
        if let Ok((_, mut driver, _)) = drivers.get_mut(m.driver_entity) {
            driver.state = DriverState::Evaluating;
            driver.matched_rider = Some(m.rider_entity);
        }
        clock.schedule_in_secs(
            1,
            EventKind::MatchAccepted,
            Some(EventSubject::Driver(m.driver_entity)),
        );
    }

    // Schedule next batch run
    clock.schedule_in_secs(config.interval_secs, EventKind::BatchMatchRun, None);
}
