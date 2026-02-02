use bevy_ecs::prelude::{Query, ResMut};

use crate::clock::{Event, EventKind, SimulationClock};
use crate::ecs::{Driver, DriverState};

fn logit_accepts(score: f64) -> bool {
    let probability = 1.0 / (1.0 + (-score).exp());
    probability >= 0.5
}

pub fn driver_decision_system(
    mut clock: ResMut<SimulationClock>,
    mut drivers: Query<&mut Driver>,
) {
    let event = match clock.pop_next() {
        Some(event) => event,
        None => return,
    };

    if event.kind != EventKind::DriverDecision {
        return;
    }

    for mut driver in drivers.iter_mut() {
        if driver.state == DriverState::Evaluating {
            if logit_accepts(1.0) {
                driver.state = DriverState::EnRoute;
                let next_timestamp = clock.now() + 1;
                clock.schedule(Event {
                    timestamp: next_timestamp,
                    kind: EventKind::TripStarted,
                });
            } else {
                driver.state = DriverState::Idle;
                driver.matched_rider = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    #[test]
    fn evaluating_driver_moves_to_en_route() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.spawn(Driver {
            state: DriverState::Evaluating,
            matched_rider: None,
        });
        world
            .resource_mut::<SimulationClock>()
            .schedule(Event {
                timestamp: 1,
                kind: EventKind::DriverDecision,
            });

        let mut schedule = Schedule::default();
        schedule.add_systems(driver_decision_system);
        schedule.run(&mut world);

        let driver = world.query::<&Driver>().single(&world);
        assert_eq!(driver.state, DriverState::EnRoute);

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("trip started event");
        assert_eq!(next_event.kind, EventKind::TripStarted);
        assert_eq!(next_event.timestamp, 2);
    }
}
