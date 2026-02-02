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

    let next_timestamp = clock.now() + 1;
    clock.schedule(Event {
        timestamp: next_timestamp,
        kind: EventKind::DriverDecision,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    use crate::clock::Event;

    #[test]
    fn match_accepted_schedules_driver_decision() {
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

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("driver decision event");
        assert_eq!(next_event.kind, EventKind::DriverDecision);
        assert_eq!(next_event.timestamp, 3);
    }
}
