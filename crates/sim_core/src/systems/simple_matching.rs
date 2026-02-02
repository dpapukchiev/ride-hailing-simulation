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
    }
    if let Ok((_, mut driver)) = drivers.get_mut(driver_entity) {
        driver.state = DriverState::Assigned;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    #[test]
    fn matches_waiting_rider_to_idle_driver() {
        let mut world = World::new();
        world.spawn(Rider {
            state: RiderState::WaitingForMatch,
        });
        world.spawn(Driver {
            state: DriverState::Idle,
        });

        let mut schedule = Schedule::default();
        schedule.add_systems(simple_matching_system);
        schedule.run(&mut world);

        let rider_state = {
            let rider = world.query::<&Rider>().single(&world);
            rider.state
        };
        let driver_state = {
            let driver = world.query::<&Driver>().single(&world);
            driver.state
        };

        assert_eq!(rider_state, RiderState::Matched);
        assert_eq!(driver_state, DriverState::Assigned);
    }
}
