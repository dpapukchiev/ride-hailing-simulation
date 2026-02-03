use bevy_ecs::prelude::{Commands, Query, Res};

use crate::clock::{CurrentEvent, EventKind, EventSubject};
use crate::ecs::{Driver, DriverState, Rider, RiderState, Trip, TripState};

pub fn rider_cancel_system(
    event: Res<CurrentEvent>,
    mut commands: Commands,
    mut riders: Query<&mut Rider>,
    mut drivers: Query<&mut Driver>,
    mut trips: Query<&mut Trip>,
) {
    if event.0.kind != EventKind::RiderCancel {
        return;
    }

    let Some(EventSubject::Rider(rider_entity)) = event.0.subject else {
        return;
    };
    let Ok(mut rider) = riders.get_mut(rider_entity) else {
        return;
    };
    if rider.state != RiderState::Waiting {
        return;
    }
    let Some(driver_entity) = rider.matched_driver else {
        return;
    };

    let mut canceled = false;
    for mut trip in trips.iter_mut() {
        if trip.rider == rider_entity && trip.state == TripState::EnRoute {
            trip.state = TripState::Cancelled;
            canceled = true;
            break;
        }
    }
    if !canceled {
        return;
    }

    if let Ok(mut driver) = drivers.get_mut(driver_entity) {
        if driver.state == DriverState::EnRoute {
            driver.state = DriverState::Idle;
        }
        driver.matched_rider = None;
    }

    rider.state = RiderState::Cancelled;
    rider.matched_driver = None;
    commands.entity(rider_entity).despawn();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};
    use bevy_ecs::schedule::apply_deferred;

    use crate::clock::SimulationClock;
    use crate::ecs::{Position, Rider, Trip};

    #[test]
    fn rider_cancel_resets_driver_and_trip() {
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
                dropoff_at: None,
            })
            .id();

        {
            let mut rider_entity_mut = world.entity_mut(rider_entity);
            let mut rider = rider_entity_mut.get_mut::<Rider>().expect("rider");
            rider.matched_driver = Some(driver_entity);
        }

        world
            .resource_mut::<SimulationClock>()
            .schedule_at_secs(1, EventKind::RiderCancel, Some(EventSubject::Rider(rider_entity)));
        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("rider cancel event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems((rider_cancel_system, apply_deferred));
        schedule.run(&mut world);

        let rider_exists = world.get_entity(rider_entity).is_some();
        assert!(!rider_exists, "rider should be despawned on cancel");

        let driver = world.entity(driver_entity).get::<Driver>().expect("driver");
        assert_eq!(driver.state, DriverState::Idle);
        assert_eq!(driver.matched_rider, None);

        let trip = world.entity(trip_entity).get::<Trip>().expect("trip");
        assert_eq!(trip.state, TripState::Cancelled);
    }
}
