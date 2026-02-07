use bevy_ecs::prelude::{Entity, Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Driver, DriverState, Position, Rider, RiderState};
use crate::matching::MatchingAlgorithmResource;
use crate::scenario::{BatchMatchingConfig, MatchRadius};

const MATCH_RETRY_SECS: u64 = 30;

pub fn matching_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    batch_config: Option<Res<BatchMatchingConfig>>,
    match_radius: Option<Res<MatchRadius>>,
    matching_algorithm: Res<MatchingAlgorithmResource>,
    mut riders: Query<(Entity, &mut Rider, &Position)>,
    mut drivers: Query<(Entity, &mut Driver, &Position)>,
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
        let Ok((_entity, rider, position)) = riders.get(rider_entity) else {
            return;
        };
        if rider.state != RiderState::Waiting {
            return;
        }
        (position.0, rider.destination)
    };

    let radius = match_radius.as_deref().map(|r| r.0).unwrap_or(0);

    // Collect available drivers (idle drivers only; exclude OffDuty drivers)
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
    if let Ok((_entity, mut rider, _)) = riders.get_mut(rider_entity) {
        rider.matched_driver = Some(driver_entity);
    }
    if let Ok((_entity, mut driver, _)) = drivers.get_mut(driver_entity) {
        driver.state = DriverState::Evaluating;
        driver.matched_rider = Some(rider_entity);
    }

    clock.schedule_in_secs(
        1,
        EventKind::MatchAccepted,
        Some(EventSubject::Driver(driver_entity)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching::{MatchingAlgorithmResource, SimpleMatching};
    use bevy_ecs::prelude::{Schedule, World};

    #[test]
    fn matches_waiting_rider_to_idle_driver() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(MatchingAlgorithmResource::new(Box::new(SimpleMatching)));
        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
        let destination = cell
            .grid_disk::<Vec<_>>(1)
            .into_iter()
            .find(|c| *c != cell)
            .expect("neighbor cell");

        let rider_entity = world
            .spawn((
                Rider {
                    state: RiderState::Waiting,
                    matched_driver: None,
                    assigned_trip: None,
                    destination: Some(destination),
                    requested_at: None,
                    quote_rejections: 0,
                    accepted_fare: None,
                    last_rejection_reason: None,
                },
                Position(cell),
            ))
            .id();
        let driver_entity = world
            .spawn((
                Driver {
                    state: DriverState::Idle,
                    matched_rider: None,
                    assigned_trip: None,
                },
                Position(cell),
            ))
            .id();

        world.resource_mut::<SimulationClock>().schedule_at_secs(
            0,
            EventKind::TryMatch,
            Some(EventSubject::Rider(rider_entity)),
        );
        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("try match event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems(matching_system);
        schedule.run(&mut world);

        let (rider_state, matched_driver) = {
            let rider = world.query::<&Rider>().single(&world);
            (rider.state, rider.matched_driver)
        };
        let (driver_state, matched_rider) = {
            let driver = world.query::<&Driver>().single(&world);
            (driver.state, driver.matched_rider)
        };

        assert_eq!(rider_state, RiderState::Waiting);
        assert_eq!(driver_state, DriverState::Evaluating);
        assert_eq!(matched_driver, Some(driver_entity));
        assert_eq!(matched_rider, Some(rider_entity));

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("trip started event");
        assert_eq!(next_event.kind, EventKind::MatchAccepted);
        assert_eq!(next_event.timestamp, crate::clock::ONE_SEC_MS);
        assert_eq!(
            next_event.subject,
            Some(EventSubject::Driver(driver_entity))
        );
    }
}
