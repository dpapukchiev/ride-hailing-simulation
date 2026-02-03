use bevy_ecs::prelude::{Commands, Query, Res, ResMut};
use rand::Rng;

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Driver, DriverState, Position, Rider, RiderState, Trip, TripState};
use crate::scenario::RiderCancelConfig;

fn logit_accepts(score: f64) -> bool {
    let probability = 1.0 / (1.0 + (-score).exp());
    probability >= 0.5
}

fn default_dropoff(pickup: h3o::CellIndex) -> h3o::CellIndex {
    pickup
        .grid_disk::<Vec<_>>(1)
        .into_iter()
        .find(|c| *c != pickup)
        .unwrap_or(pickup)
}

pub fn driver_decision_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    mut commands: Commands,
    mut drivers: Query<&mut Driver>,
    riders: Query<(&Rider, &Position)>,
    cancel_config: Option<Res<RiderCancelConfig>>,
) {
    if event.0.kind != EventKind::DriverDecision {
        return;
    }

    let Some(EventSubject::Driver(driver_entity)) = event.0.subject else {
        return;
    };
    let Ok(mut driver) = drivers.get_mut(driver_entity) else {
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

        let (pickup, dropoff, requested_at, rider_waiting) = match riders.get(rider_entity) {
            Ok((rider, pos)) => {
                let pickup = pos.0;
                let dropoff = rider
                    .destination
                    .unwrap_or_else(|| default_dropoff(pickup));
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
            return;
        }

        let matched_at = clock.now();
        driver.state = DriverState::EnRoute;
        let trip_entity = commands
            .spawn(Trip {
                state: TripState::EnRoute,
                rider: rider_entity,
                driver: driver_entity,
                pickup,
                dropoff,
                requested_at,
                matched_at,
                pickup_at: None,
                dropoff_at: None,
            })
            .id();

        clock.schedule_in_secs(1, EventKind::MoveStep, Some(EventSubject::Trip(trip_entity)));
        let config = cancel_config
            .as_deref()
            .copied()
            .unwrap_or_default();
        let min_wait_secs = config.min_wait_secs;
        let max_wait_secs = config.max_wait_secs.max(min_wait_secs);
        let mut rng = rand::thread_rng();
        let wait_secs = rng.gen_range(min_wait_secs..=max_wait_secs);
        clock.schedule_in_secs(
            wait_secs,
            EventKind::RiderCancel,
            Some(EventSubject::Rider(rider_entity)),
        );
    } else {
        driver.state = DriverState::Idle;
        driver.matched_rider = None;
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
        let rider_entity = world
            .spawn((
                Rider {
                    state: RiderState::Waiting,
                    matched_driver: None,
                    destination: None,
                    requested_at: None,
                },
                Position(cell),
            ))
            .id();
        let driver_entity = world
            .spawn(Driver {
            state: DriverState::Evaluating,
            matched_rider: Some(rider_entity),
        })
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
        // dropoff is a neighbor of pickup when destination is None
    }
}
