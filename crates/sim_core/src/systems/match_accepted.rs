use bevy_ecs::prelude::{Query, ResMut};

use crate::clock::{Event, EventKind, SimulationClock};
use crate::ecs::{Driver, DriverState};

pub fn match_accepted_system(
    mut clock: ResMut<SimulationClock>,
    mut drivers: Query<&mut Driver>,
) {
    let event = match clock.pop_next() {
        Some(event) => event,
        None => return,
    };

    if event.kind != EventKind::MatchAccepted {
        return;
    }

    for mut driver in drivers.iter_mut() {
        if driver.state == DriverState::Evaluating {
            driver.state = DriverState::EnRoute;
        }
    }

    let next_timestamp = clock.now() + 1;
    clock.schedule(Event {
        timestamp: next_timestamp,
        kind: EventKind::TripStarted,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    use crate::clock::Event;

    #[test]
    fn match_accepted_transitions_and_schedules_trip_start() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.spawn(Driver {
            state: DriverState::Evaluating,
            matched_rider: None,
        });

        world
            .resource_mut::<SimulationClock>()
            .schedule(Event {
                timestamp: 2,
                kind: EventKind::MatchAccepted,
            });

        let mut schedule = Schedule::default();
        schedule.add_systems(match_accepted_system);
        schedule.run(&mut world);

        let driver_state = {
            let driver = world.query::<&Driver>().single(&world);
            driver.state
        };
        assert_eq!(driver_state, DriverState::EnRoute);

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("trip started event");
        assert_eq!(next_event.kind, EventKind::TripStarted);
        assert_eq!(next_event.timestamp, 3);
    }
}
