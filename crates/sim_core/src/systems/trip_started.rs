use bevy_ecs::prelude::{Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Driver, DriverState, Position, Rider, RiderState, Trip, TripState};

pub fn trip_started_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    mut trips: Query<&mut Trip>,
    mut riders: Query<(&mut Rider, &Position)>,
    mut drivers: Query<(&mut Driver, &Position)>,
) {
    if event.0.kind != EventKind::TripStarted {
        return;
    }

    let Some(EventSubject::Trip(trip_entity)) = event.0.subject else {
        return;
    };

    let (driver_entity, rider_entity) = {
        let Ok(trip) = trips.get(trip_entity) else {
            return;
        };
        if trip.state != TripState::EnRoute {
            return;
        }
        (trip.driver, trip.rider)
    };

    let driver_pos = {
        let Ok((driver, driver_pos)) = drivers.get(driver_entity) else {
            return;
        };
        if driver.state != DriverState::EnRoute {
            return;
        }
        driver_pos.0
    };

    let (rider_pos, rider_matched_driver_ok, rider_waiting) = {
        let Ok((rider, rider_pos)) = riders.get(rider_entity) else {
            return;
        };
        (
            rider_pos.0,
            rider.matched_driver == Some(driver_entity),
            rider.state == RiderState::Waiting,
        )
    };
    if !rider_matched_driver_ok || !rider_waiting || rider_pos != driver_pos {
        return;
    }

    if let Ok((mut rider, _)) = riders.get_mut(rider_entity) {
        rider.state = RiderState::InTransit;
    }
    if let Ok((mut driver, _)) = drivers.get_mut(driver_entity) {
        driver.state = DriverState::OnTrip;
    }
    if let Ok(mut trip) = trips.get_mut(trip_entity) {
        trip.state = TripState::OnTrip;
        trip.pickup_at = Some(clock.now());
    }

    // Start moving driver toward dropoff; completion is scheduled by movement when driver arrives.
    clock.schedule_in_secs(1, EventKind::MoveStep, Some(EventSubject::Trip(trip_entity)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    #[test]
    fn trip_started_transitions_and_schedules_completion() {
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
            .spawn((
                Driver {
                    state: DriverState::EnRoute,
                    matched_rider: Some(rider_entity),
                },
                Position(cell),
            ))
            .id();
        let trip_entity = world
            .spawn(Trip {
                state: TripState::EnRoute,
                rider: rider_entity,
                driver: driver_entity,
                pickup: cell,
                dropoff: cell,
                requested_at: 0,
                matched_at: 0,
                pickup_at: None,
            })
            .id();

        {
            let mut rider_entity_mut = world.entity_mut(rider_entity);
            let mut rider = rider_entity_mut.get_mut::<Rider>().expect("rider");
            rider.matched_driver = Some(driver_entity);
        }

        world
            .resource_mut::<SimulationClock>()
            .schedule_at_secs(3, EventKind::TripStarted, Some(EventSubject::Trip(trip_entity)));

        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("trip started event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems(trip_started_system);
        schedule.run(&mut world);

        let rider_state = {
            let rider = world.query::<&Rider>().single(&world);
            rider.state
        };
        let driver_state = {
            let driver = world.query::<&Driver>().single(&world);
            driver.state
        };

        assert_eq!(rider_state, RiderState::InTransit);
        assert_eq!(driver_state, DriverState::OnTrip);
        let trip_state = world.entity(trip_entity).get::<Trip>().expect("trip").state;
        assert_eq!(trip_state, TripState::OnTrip);

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("move step toward dropoff");
        assert_eq!(next_event.kind, EventKind::MoveStep);
        assert_eq!(next_event.timestamp, 4000);
        assert_eq!(next_event.subject, Some(EventSubject::Trip(trip_entity)));
    }
}
