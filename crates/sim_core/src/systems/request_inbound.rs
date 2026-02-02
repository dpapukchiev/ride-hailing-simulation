use bevy_ecs::prelude::{Query, ResMut};

use crate::clock::{EventKind, SimulationClock};
use crate::ecs::{Rider, RiderState};

pub fn request_inbound_system(
    mut clock: ResMut<SimulationClock>,
    mut riders: Query<&mut Rider>,
) {
    let event = match clock.pop_next() {
        Some(event) => event,
        None => return,
    };

    if event.kind != EventKind::RequestInbound {
        return;
    }

    for mut rider in riders.iter_mut() {
        if rider.state == RiderState::Requesting {
            rider.state = RiderState::WaitingForMatch;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    use crate::clock::Event;

    #[test]
    fn ecs_system_transitions_rider_state() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.spawn(Rider {
            state: RiderState::Requesting,
            matched_driver: None,
        });

        world
            .resource_mut::<SimulationClock>()
            .schedule(Event {
                timestamp: 1,
                kind: EventKind::RequestInbound,
            });

        let mut schedule = Schedule::default();
        schedule.add_systems(request_inbound_system);
        schedule.run(&mut world);

        let rider = world.query::<&Rider>().single(&world);
        assert_eq!(rider.state, RiderState::WaitingForMatch);
    }
}
