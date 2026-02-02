use bevy_ecs::prelude::{Entity, Query};

use crate::ecs::{Driver, DriverState, Rider, RiderState};

pub fn simple_matching_system(
    mut riders: Query<(Entity, &mut Rider)>,
    mut drivers: Query<(Entity, &mut Driver)>,
) {
    let rider_entity = riders
        .iter()
        .find(|(_, rider)| rider.state == RiderState::WaitingForMatch)
        .map(|(entity, _)| entity);
    let driver_entity = drivers
        .iter()
        .find(|(_, driver)| driver.state == DriverState::Idle)
        .map(|(entity, _)| entity);

    let (Some(rider_entity), Some(driver_entity)) = (rider_entity, driver_entity) else {
        return;
    };

    if let Ok((_, mut rider)) = riders.get_mut(rider_entity) {
        rider.state = RiderState::Matched;
        rider.matched_driver = Some(driver_entity);
    }
    if let Ok((_, mut driver)) = drivers.get_mut(driver_entity) {
        driver.state = DriverState::Assigned;
        driver.matched_rider = Some(rider_entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    #[test]
    fn matches_waiting_rider_to_idle_driver() {
        let mut world = World::new();
        let rider_entity = world.spawn(Rider {
            state: RiderState::WaitingForMatch,
            matched_driver: None,
        }).id();
        let driver_entity = world.spawn(Driver {
            state: DriverState::Idle,
            matched_rider: None,
        }).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(simple_matching_system);
        schedule.run(&mut world);

        let (rider_state, matched_driver) = {
            let rider = world.query::<&Rider>().single(&world);
            (rider.state, rider.matched_driver)
        };
        let (driver_state, matched_rider) = {
            let driver = world.query::<&Driver>().single(&world);
            (driver.state, driver.matched_rider)
        };

        assert_eq!(rider_state, RiderState::Matched);
        assert_eq!(driver_state, DriverState::Assigned);
        assert_eq!(matched_driver, Some(driver_entity));
        assert_eq!(matched_rider, Some(rider_entity));
    }
}
