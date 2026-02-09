use bevy_ecs::prelude::{Commands, Entity, Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Driver, DriverStateCommands, Idle, Position, Rider, Waiting};
use crate::matching::MatchingAlgorithmResource;
use crate::scenario::{BatchMatchingConfig, MatchRadius};

const MATCH_RETRY_SECS: u64 = 30;

pub fn matching_system(
    mut commands: Commands,
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    batch_config: Option<Res<BatchMatchingConfig>>,
    match_radius: Option<Res<MatchRadius>>,
    matching_algorithm: Res<MatchingAlgorithmResource>,
    mut riders: Query<(Entity, &mut Rider, &Position, Option<&Waiting>)>,
    mut drivers: Query<(Entity, &mut Driver, &Position, Option<&Idle>)>,
) {
    if event.0.kind != EventKind::TryMatch {
        return;
    }
    // When batch matching is enabled, per-rider matching is not used
    if batch_config.as_deref().is_some_and(|c| c.enabled) {
        return;
    }

    let Some(EventSubject::Rider(rider_entity)) = event.0.subject else {
        return;
    };

    let (rider_pos, rider_destination) = {
        let Ok((_entity, rider, position, waiting)) = riders.get(rider_entity) else {
            return;
        };
        if waiting.is_none() {
            return;
        }
        (position.0, rider.destination)
    };

    let radius = match_radius.as_deref().map(|r| r.0).unwrap_or(0);

    // Collect available drivers (idle drivers only; exclude OffDuty drivers)
    let available_drivers: Vec<(Entity, h3o::CellIndex)> = drivers
        .iter()
        .filter_map(|(entity, _driver, position, idle)| idle.map(|_| (entity, position.0)))
        .collect();

    // Use the matching algorithm to find a match
    let driver_entity = matching_algorithm.find_match(
        rider_entity,
        rider_pos,
        rider_destination,
        &available_drivers,
        radius,
        clock.now(),
    );

    let Some(driver_entity) = driver_entity else {
        clock.schedule_in_secs(
            MATCH_RETRY_SECS,
            EventKind::TryMatch,
            Some(EventSubject::Rider(rider_entity)),
        );
        return;
    };

    // Apply the match
    if let Ok((_entity, mut rider, _, _)) = riders.get_mut(rider_entity) {
        rider.matched_driver = Some(driver_entity);
    }
    if let Ok((_entity, mut driver, _, _)) = drivers.get_mut(driver_entity) {
        commands.entity(driver_entity).set_driver_state_evaluating();
        driver.matched_rider = Some(rider_entity);
    }

    clock.schedule_in_secs(
        1,
        EventKind::MatchAccepted,
        Some(EventSubject::Driver(driver_entity)),
    );
}
