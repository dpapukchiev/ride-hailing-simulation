use bevy_ecs::prelude::{Query, Res};

use crate::clock::{CurrentEvent, EventKind, EventSubject};
use crate::ecs::{Driver, DriverState, Rider, RiderState};

pub fn trip_completed_system(
    event: Res<CurrentEvent>,
    mut riders: Query<&mut Rider>,
    mut drivers: Query<&mut Driver>,
) {
    if event.0.kind != EventKind::TripCompleted {
        return;
    }

    let Some(EventSubject::Driver(driver_entity)) = event.0.subject else {
        return;
    };
    let Ok(mut driver) = drivers.get_mut(driver_entity) else {
        return;
    };
    if driver.state != DriverState::OnTrip {
        return;
    }

    let rider_entity = driver.matched_rider;
    driver.state = DriverState::Idle;
    driver.matched_rider = None;

    let Some(rider_entity) = rider_entity else {
        return;
    };
    let Ok(mut rider) = riders.get_mut(rider_entity) else {
        return;
    };
    if rider.state == RiderState::InTransit {
        rider.state = RiderState::Completed;
        rider.matched_driver = None;
    }
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
        let rider_entity = world
            .spawn(Rider {
                state: RiderState::InTransit,
                matched_driver: None,
            })
            .id();
        let driver_entity = world
            .spawn(Driver {
                state: DriverState::OnTrip,
                matched_rider: None,
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
                subject: Some(EventSubject::Driver(driver_entity)),
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
    }
}
