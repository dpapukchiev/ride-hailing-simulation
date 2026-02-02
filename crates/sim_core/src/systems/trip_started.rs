use bevy_ecs::prelude::{Query, ResMut};

use crate::clock::{Event, EventKind, SimulationClock};
use crate::ecs::{Driver, DriverState, Rider, RiderState};

pub fn trip_started_system(
    mut clock: ResMut<SimulationClock>,
    mut riders: Query<&mut Rider>,
    mut drivers: Query<&mut Driver>,
) {
    let event = match clock.pop_next() {
        Some(event) => event,
        None => return,
    };

    if event.kind != EventKind::TripStarted {
        return;
    }

    for mut rider in riders.iter_mut() {
        if rider.state == RiderState::Waiting {
            rider.state = RiderState::InTransit;
        }
    }

    for mut driver in drivers.iter_mut() {
        if driver.state == DriverState::EnRoute {
            driver.state = DriverState::OnTrip;
        }
    }

    let next_timestamp = clock.now() + 1;
    clock.schedule(Event {
        timestamp: next_timestamp,
        kind: EventKind::TripCompleted,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    #[test]
    fn trip_started_transitions_and_schedules_completion() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.spawn(Rider {
            state: RiderState::Waiting,
            matched_driver: None,
        });
        world.spawn(Driver {
            state: DriverState::EnRoute,
            matched_rider: None,
        });

        world
            .resource_mut::<SimulationClock>()
            .schedule(Event {
                timestamp: 3,
                kind: EventKind::TripStarted,
            });

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

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("completion event");
        assert_eq!(next_event.kind, EventKind::TripCompleted);
        assert_eq!(next_event.timestamp, 4);
    }
}
