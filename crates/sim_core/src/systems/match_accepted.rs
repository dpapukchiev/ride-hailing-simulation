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
