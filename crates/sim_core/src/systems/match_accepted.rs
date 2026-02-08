use bevy_ecs::prelude::{Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};

pub fn match_accepted_system(mut clock: ResMut<SimulationClock>, event: Res<CurrentEvent>) {
    if event.0.kind != EventKind::MatchAccepted {
        return;
    }

    let Some(EventSubject::Driver(driver_entity)) = event.0.subject else {
        return;
    };

    clock.schedule_in_secs(
        1,
        EventKind::DriverDecision,
        Some(EventSubject::Driver(driver_entity)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    use crate::ecs::{Driver, Evaluating};

    #[test]
    fn match_accepted_schedules_driver_decision() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        let driver_entity = world
            .spawn((
                Driver {
                    matched_rider: None,
                    assigned_trip: None,
                },
                Evaluating,
            ))
            .id();

        world.resource_mut::<SimulationClock>().schedule_at_secs(
            2,
            EventKind::MatchAccepted,
            Some(EventSubject::Driver(driver_entity)),
        );

        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("match accepted event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems(match_accepted_system);
        schedule.run(&mut world);

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("driver decision event");
        assert_eq!(next_event.kind, EventKind::DriverDecision);
        assert_eq!(next_event.timestamp, 3000);
        assert_eq!(
            next_event.subject,
            Some(EventSubject::Driver(driver_entity))
        );
    }
}
