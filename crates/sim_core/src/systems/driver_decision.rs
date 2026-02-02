use bevy_ecs::prelude::{Commands, Query, Res, ResMut};

use crate::clock::{CurrentEvent, Event, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Driver, DriverState, Trip, TripState};

fn logit_accepts(score: f64) -> bool {
    let probability = 1.0 / (1.0 + (-score).exp());
    probability >= 0.5
}

pub fn driver_decision_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    mut commands: Commands,
    mut drivers: Query<&mut Driver>,
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

        driver.state = DriverState::EnRoute;
        let trip_entity = commands
            .spawn(Trip {
                state: TripState::EnRoute,
                rider: rider_entity,
                driver: driver_entity,
            })
            .id();

        let next_timestamp = clock.now() + 1;
        clock.schedule(Event {
            timestamp: next_timestamp,
            kind: EventKind::MoveStep,
            subject: Some(EventSubject::Trip(trip_entity)),
        });
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

    use crate::ecs::{Rider, RiderState};

    #[test]
    fn evaluating_driver_moves_to_en_route() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        let rider_entity = world
            .spawn(Rider {
                state: RiderState::Waiting,
                matched_driver: None,
            })
            .id();
        let driver_entity = world
            .spawn(Driver {
            state: DriverState::Evaluating,
            matched_rider: Some(rider_entity),
        })
            .id();
        world
            .resource_mut::<SimulationClock>()
            .schedule(Event {
                timestamp: 1,
                kind: EventKind::DriverDecision,
                subject: Some(EventSubject::Driver(driver_entity)),
            });

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
        assert_eq!(next_event.timestamp, 2);
        let trip_entity = match next_event.subject {
            Some(EventSubject::Trip(trip_entity)) => trip_entity,
            other => panic!("expected trip subject, got {other:?}"),
        };
        let trip = world.entity(trip_entity).get::<Trip>().expect("trip");
        assert_eq!(trip.state, TripState::EnRoute);
        assert_eq!(trip.driver, driver_entity);
        assert_eq!(trip.rider, rider_entity);
    }
}
