use bevy_ecs::prelude::{Query, Res, ResMut};

use crate::clock::{CurrentEvent, Event, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Driver, DriverState};

fn logit_accepts(score: f64) -> bool {
    let probability = 1.0 / (1.0 + (-score).exp());
    probability >= 0.5
}

pub fn driver_decision_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    mut drivers: Query<&mut Driver>,
) {
    if event.0.kind != EventKind::DriverDecision {
        return;
    }

    let Some(EventSubject::Driver(driver_entity)) = event.0.subject else {
        return;
    };
    let Ok(mut driver) = drivers.get_mut(driver_entity) else {
        return;
    };
    if driver.state != DriverState::Evaluating {
        return;
    }

    if logit_accepts(1.0) {
        driver.state = DriverState::EnRoute;
        let next_timestamp = clock.now() + 1;
        clock.schedule(Event {
            timestamp: next_timestamp,
            kind: EventKind::MoveStep,
            subject: Some(EventSubject::Driver(driver_entity)),
        });
    } else {
        driver.state = DriverState::Idle;
        driver.matched_rider = None;
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
        let driver_entity = world
            .spawn(Driver {
            state: DriverState::Evaluating,
            matched_rider: None,
        })
            .id();
        world
            .resource_mut::<SimulationClock>()
            .schedule(Event {
                timestamp: 1,
                kind: EventKind::DriverDecision,
                subject: Some(EventSubject::Driver(driver_entity)),
            });

        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("driver decision event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems(driver_decision_system);
        schedule.run(&mut world);

        let driver = world.query::<&Driver>().single(&world);
        assert_eq!(driver.state, DriverState::EnRoute);

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("move step event");
        assert_eq!(next_event.kind, EventKind::MoveStep);
        assert_eq!(next_event.timestamp, 2);
        assert_eq!(next_event.subject, Some(EventSubject::Driver(driver_entity)));
    }
}
