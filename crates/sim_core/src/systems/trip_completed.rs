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
        if rider.state == RiderState::Matched {
            rider.state = RiderState::Completed;
        }
    }

    for mut driver in drivers.iter_mut() {
        if driver.state == DriverState::Assigned {
            driver.state = DriverState::Idle;
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
        world.spawn(Rider {
            state: RiderState::Matched,
        });
        world.spawn(Driver {
            state: DriverState::Assigned,
        });

        world
            .resource_mut::<SimulationClock>()
            .schedule(Event {
                timestamp: 2,
                kind: EventKind::TripCompleted,
            });

        let mut schedule = Schedule::default();
        schedule.add_systems(trip_completed_system);
        schedule.run(&mut world);

        let rider_state = {
            let rider = world.query::<&Rider>().single(&world);
            rider.state
        };
        let driver_state = {
            let driver = world.query::<&Driver>().single(&world);
            driver.state
        };

        assert_eq!(rider_state, RiderState::Completed);
        assert_eq!(driver_state, DriverState::Idle);
    }
}
