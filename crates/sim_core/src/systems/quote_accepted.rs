use bevy_ecs::prelude::{Query, ResMut};

use crate::clock::{EventKind, SimulationClock};
use crate::ecs::{Rider, RiderState};

pub fn quote_accepted_system(mut clock: ResMut<SimulationClock>, mut riders: Query<&mut Rider>) {
    let event = match clock.pop_next() {
        Some(event) => event,
        None => return,
    };

    if event.kind != EventKind::QuoteAccepted {
        return;
    }

    for mut rider in riders.iter_mut() {
        if rider.state == RiderState::Browsing {
            rider.state = RiderState::Waiting;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    use crate::clock::Event;

    #[test]
    fn quote_accepted_transitions_rider_state() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.spawn(Rider {
            state: RiderState::Browsing,
            matched_driver: None,
        });

        world
            .resource_mut::<SimulationClock>()
            .schedule(Event {
                timestamp: 1,
                kind: EventKind::QuoteAccepted,
            });

        let mut schedule = Schedule::default();
        schedule.add_systems(quote_accepted_system);
        schedule.run(&mut world);

        let rider = world.query::<&Rider>().single(&world);
        assert_eq!(rider.state, RiderState::Waiting);
    }
}
