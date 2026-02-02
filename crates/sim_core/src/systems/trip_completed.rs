use bevy_ecs::prelude::{Query, ResMut};

use crate::clock::{EventKind, SimulationClock};
use crate::ecs::{Driver, DriverState, Rider, RiderState};

pub fn trip_completed_system(
    mut clock: ResMut<SimulationClock>,
    mut riders: Query<&mut Rider>,
    mut drivers: Query<&mut Driver>,
) {
    let event = match clock.pop_next() {
        Some(event) => event,
        None => return,
    };

    if event.kind != EventKind::TripCompleted {
        return;
    }

    for mut rider in riders.iter_mut() {
        if rider.state == RiderState::InTransit {
            rider.state = RiderState::Completed;
            rider.matched_driver = None;
        }
    }

    for mut driver in drivers.iter_mut() {
        if driver.state == DriverState::OnTrip {
            driver.state = DriverState::Idle;
            driver.matched_rider = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    use crate::clock::Event;

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
            });

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
