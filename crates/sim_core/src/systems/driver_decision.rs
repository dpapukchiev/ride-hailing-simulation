use bevy_ecs::prelude::{Commands, Entity, Query, Res, ResMut};
use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Driver, DriverState, Position, Rider, RiderState, Trip, TripState};
use crate::scenario::BatchMatchingConfig;
use crate::spatial::distance_km_between_cells;

const MATCH_RETRY_SECS: u64 = 30;

fn logit_accepts(score: f64) -> bool {
    let probability = 1.0 / (1.0 + (-score).exp());
    probability >= 0.5
}

pub fn driver_decision_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    batch_config: Option<Res<BatchMatchingConfig>>,
    mut commands: Commands,
    mut drivers: Query<(&mut Driver, &Position)>,
    mut riders: Query<(Entity, &mut Rider, &Position)>,
) {
    if event.0.kind != EventKind::DriverDecision {
        return;
    }

    let Some(EventSubject::Driver(driver_entity)) = event.0.subject else {
        return;
    };
    let Ok((mut driver, driver_pos)) = drivers.get_mut(driver_entity) else {
        return;
    };
    if driver.state != DriverState::Evaluating {
        return;
    }

    if logit_accepts(1.0) {
        let Some(rider_entity) = driver.matched_rider else {
            driver.state = DriverState::Idle;
            return;
        };

        let (pickup, dropoff, requested_at, rider_waiting) = match riders.get_mut(rider_entity) {
            Ok((_entity, rider, pos)) => {
                let pickup = pos.0;
                let Some(dropoff) = rider.destination else {
                    // Rider has no destination; reset driver and bail
                    driver.state = DriverState::Idle;
                    driver.matched_rider = None;
                    return;
                };
                let requested_at = rider.requested_at.unwrap_or(clock.now());
                (pickup, dropoff, requested_at, rider.state == RiderState::Waiting)
            }
            Err(_) => {
                driver.state = DriverState::Idle;
                return;
            }
        };
        if !rider_waiting {
            driver.state = DriverState::Idle;
            driver.matched_rider = None;
            if let Ok((_entity, mut rider, _)) = riders.get_mut(rider_entity) {
                rider.matched_driver = None;
            }
            return;
        }

        let matched_at = clock.now();
        let pickup_distance_km_at_accept = distance_km_between_cells(driver_pos.0, pickup);
        driver.state = DriverState::EnRoute;
        let agreed_fare = riders
            .get(rider_entity)
            .ok()
            .and_then(|(_, r, _)| r.accepted_fare);

        let trip_entity = commands
            .spawn(Trip {
                state: TripState::EnRoute,
                rider: rider_entity,
                driver: driver_entity,
                pickup,
                dropoff,
                pickup_distance_km_at_accept,
                requested_at,
                matched_at,
                pickup_at: None,
                pickup_eta_ms: 0,
                dropoff_at: None,
                cancelled_at: None,
                agreed_fare,
            })
            .id();

        clock.schedule_in_secs(1, EventKind::MoveStep, Some(EventSubject::Trip(trip_entity)));
    } else {
        let rejected_rider = driver.matched_rider;
        driver.state = DriverState::Idle;
        driver.matched_rider = None;
        if let Some(rider_entity) = rejected_rider {
            if let Ok((_entity, mut rider, _)) = riders.get_mut(rider_entity) {
                rider.matched_driver = None;
                // Only schedule per-rider TryMatch when batch matching is disabled
                let batch_enabled = batch_config.as_deref().map_or(false, |c| c.enabled);
                if rider.state == RiderState::Waiting && !batch_enabled {
                    clock.schedule_in_secs(
                        MATCH_RETRY_SECS,
                        EventKind::TryMatch,
                        Some(EventSubject::Rider(rider_entity)),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::apply_deferred;
    use bevy_ecs::prelude::{Schedule, World};

    use crate::ecs::{Position, Rider, RiderState};

    #[test]
    fn evaluating_driver_moves_to_en_route() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
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
                    destination: Some(destination),
                    requested_at: None,
                    quote_rejections: 0,
                    accepted_fare: Some(15.0),
                },
                Position(cell),
            ))
            .id();
        let driver_entity = world
            .spawn((
                Driver {
                    state: DriverState::Evaluating,
                    matched_rider: Some(rider_entity),
                },
                Position(cell),
            ))
            .id();
        world
            .resource_mut::<SimulationClock>()
            .schedule_at_secs(1, EventKind::DriverDecision, Some(EventSubject::Driver(driver_entity)));

        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("driver decision event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems((driver_decision_system, apply_deferred));
        schedule.run(&mut world);

        let driver = world.query::<&Driver>().single(&world);
        assert_eq!(driver.state, DriverState::EnRoute);

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("move step event");
        assert_eq!(next_event.kind, EventKind::MoveStep);
        assert_eq!(next_event.timestamp, 2000);
        let trip_entity = match next_event.subject {
            Some(EventSubject::Trip(trip_entity)) => trip_entity,
            other => panic!("expected trip subject, got {other:?}"),
        };
        let trip = world.entity(trip_entity).get::<Trip>().expect("trip");
        assert_eq!(trip.state, TripState::EnRoute);
        assert_eq!(trip.driver, driver_entity);
        assert_eq!(trip.rider, rider_entity);
        assert_eq!(trip.pickup, cell);
        assert_eq!(trip.dropoff, destination);
    }
}
