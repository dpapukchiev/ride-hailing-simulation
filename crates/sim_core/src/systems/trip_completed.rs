use bevy_ecs::prelude::{Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Driver, DriverState, Rider, RiderState, Trip, TripState};
use crate::telemetry::{CompletedTripRecord, SimTelemetry};

pub fn trip_completed_system(
    event: Res<CurrentEvent>,
    clock: Res<SimulationClock>,
    mut telemetry: ResMut<SimTelemetry>,
    mut trips: Query<&mut Trip>,
    mut riders: Query<&mut Rider>,
    mut drivers: Query<&mut Driver>,
) {
    if event.0.kind != EventKind::TripCompleted {
        return;
    }

    let Some(EventSubject::Trip(trip_entity)) = event.0.subject else {
        return;
    };

    let Ok(mut trip) = trips.get_mut(trip_entity) else {
        return;
    };
    if trip.state != TripState::OnTrip {
        return;
    }

    let driver_entity = trip.driver;
    let rider_entity = trip.rider;

    if let Ok(mut driver) = drivers.get_mut(driver_entity) {
        if driver.state == DriverState::OnTrip {
            driver.state = DriverState::Idle;
        }
        driver.matched_rider = None;
    }

    if let Ok(mut rider) = riders.get_mut(rider_entity) {
        if rider.state == RiderState::InTransit {
            rider.state = RiderState::Completed;
        }
        rider.matched_driver = None;
    }

    trip.state = TripState::Completed;

    telemetry.completed_trips.push(CompletedTripRecord {
        trip_entity,
        rider_entity,
        driver_entity,
        completed_at: clock.now(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    use crate::clock::{Event, SimulationClock};

    #[test]
    fn trip_completed_transitions_driver_and_rider() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(crate::telemetry::SimTelemetry::default());
        let rider_entity = world
            .spawn(Rider {
                state: RiderState::InTransit,
                matched_driver: None,
                destination: None,
            })
            .id();
        let driver_entity = world
            .spawn(Driver {
                state: DriverState::OnTrip,
                matched_rider: None,
            })
            .id();
        let trip_entity = world
            .spawn(Trip {
                state: TripState::OnTrip,
                rider: rider_entity,
                driver: driver_entity,
                pickup: h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell"),
                dropoff: h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell"),
            })
            .id();

        {
            let mut rider_entity_mut = world.entity_mut(rider_entity);
            let mut rider = rider_entity_mut.get_mut::<Rider>().expect("rider");
            rider.matched_driver = Some(driver_entity);
        }
        {
            let mut driver_entity_mut = world.entity_mut(driver_entity);
            let mut driver = driver_entity_mut.get_mut::<Driver>().expect("driver");
            driver.matched_rider = Some(rider_entity);
        }

        world
            .resource_mut::<SimulationClock>()
            .schedule(Event {
                timestamp: 2,
                kind: EventKind::TripCompleted,
                subject: Some(EventSubject::Trip(trip_entity)),
            });

        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("trip completed event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems(trip_completed_system);
        schedule.run(&mut world);

        let (rider_state, matched_driver) = {
            let rider = world.query::<&Rider>().single(&world);
            (rider.state, rider.matched_driver)
        };
        let (driver_state, matched_rider) = {
            let driver = world.query::<&Driver>().single(&world);
            (driver.state, driver.matched_rider)
        };

        assert_eq!(rider_state, RiderState::Completed);
        assert_eq!(driver_state, DriverState::Idle);
        assert_eq!(matched_driver, None);
        assert_eq!(matched_rider, None);

        let trip_state = world.entity(trip_entity).get::<Trip>().expect("trip").state;
        assert_eq!(trip_state, TripState::Completed);
    }
}
