use bevy_ecs::prelude::{Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock, ONE_SEC_MS};
use crate::ecs::{Driver, DriverState, Position, Trip, TripState};
use crate::spatial::distance_km_between_cells;
use crate::speed::{SpeedFactors, SpeedModel};

fn travel_time_ms(distance_km: f64, speed_kmh: f64) -> u64 {
    if distance_km <= 0.0 {
        ONE_SEC_MS
    } else {
        let hours = distance_km / speed_kmh.max(1.0);
        let ms = (hours * 60.0 * 60.0 * 1000.0).round() as u64;
        ms.max(ONE_SEC_MS)
    }
}

pub fn movement_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    mut speed: ResMut<SpeedModel>,
    mut trips: Query<&mut Trip>,
    mut drivers: Query<(&mut Driver, &mut Position)>,
) {
    if event.0.kind != EventKind::MoveStep {
        return;
    }

    let Some(EventSubject::Trip(trip_entity)) = event.0.subject else {
        return;
    };

    let (driver_entity, target_cell, is_en_route) = {
        let Ok(trip) = trips.get(trip_entity) else {
            return;
        };
        let target = match trip.state {
            TripState::EnRoute => trip.pickup,
            TripState::OnTrip => trip.dropoff,
            TripState::Completed | TripState::Cancelled => return,
        };
        (trip.driver, target, trip.state == TripState::EnRoute)
    };

    let Ok((driver, mut driver_pos)) = drivers.get_mut(driver_entity) else {
        return;
    };
    let expected_state = if is_en_route {
        DriverState::EnRoute
    } else {
        DriverState::OnTrip
    };
    if driver.state != expected_state {
        return;
    }

    let speed_kmh = speed.sample_kmh(SpeedFactors::default());
    let remaining_km = distance_km_between_cells(driver_pos.0, target_cell);
    if remaining_km <= 0.0 {
        if is_en_route {
            if let Ok(mut trip) = trips.get_mut(trip_entity) {
                trip.pickup_eta_ms = 0;
            }
        }
        let kind = if is_en_route {
            EventKind::TripStarted
        } else {
            EventKind::TripCompleted
        };
        clock.schedule_in_secs(1, kind, Some(EventSubject::Trip(trip_entity)));
        return;
    }

    let mut step_distance_km = remaining_km;
    if let Ok(path) = driver_pos.0.grid_path_cells(target_cell) {
        let mut iter = path.filter_map(|cell| cell.ok());
        let _current = iter.next();
        if let Some(next_cell) = iter.next() {
            step_distance_km = distance_km_between_cells(driver_pos.0, next_cell);
            driver_pos.0 = next_cell;
        }
    }

    let remaining = distance_km_between_cells(driver_pos.0, target_cell);
    if is_en_route {
        if let Ok(mut trip) = trips.get_mut(trip_entity) {
            trip.pickup_eta_ms = if remaining <= 0.0 {
                0
            } else {
                travel_time_ms(remaining, speed_kmh)
            };
        }
        clock.schedule_in(0, EventKind::PickupEtaUpdated, Some(EventSubject::Trip(trip_entity)));
    }
    if remaining <= 0.0 {
        let kind = if is_en_route {
            EventKind::TripStarted
        } else {
            EventKind::TripCompleted
        };
        clock.schedule_in_secs(1, kind, Some(EventSubject::Trip(trip_entity)));
    } else {
        let step_ms = travel_time_ms(step_distance_km, speed_kmh);
        clock.schedule_in(step_ms, EventKind::MoveStep, Some(EventSubject::Trip(trip_entity)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};
    use crate::clock::ONE_SEC_MS;
    use crate::ecs::{Rider, RiderState};
    use crate::speed::SpeedModel;

    #[test]
    fn movement_steps_toward_rider_and_schedules_trip_start() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(SpeedModel::with_range(Some(1), 40.0, 40.0));

        let origin = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
        let neighbor = origin
            .grid_disk::<Vec<_>>(1)
            .into_iter()
            .find(|cell| *cell != origin)
            .expect("neighbor");

        let rider_entity = world
            .spawn((
                Rider {
                    state: RiderState::Waiting,
                    matched_driver: None,
                    destination: None,
                    requested_at: None,
                },
                Position(neighbor),
            ))
            .id();
        let driver_entity = world
            .spawn((
                Driver {
                    state: DriverState::EnRoute,
                    matched_rider: Some(rider_entity),
                },
                Position(origin),
            ))
            .id();
        let trip_entity = world
            .spawn(Trip {
                state: TripState::EnRoute,
                rider: rider_entity,
                driver: driver_entity,
                pickup: neighbor,
                dropoff: neighbor,
                pickup_distance_km_at_accept: 0.0,
                requested_at: 0,
                matched_at: 0,
                pickup_at: None,
                pickup_eta_ms: 0,
                dropoff_at: None,
                cancelled_at: None,
            })
            .id();

        world
            .resource_mut::<SimulationClock>()
            .schedule_at_secs(1, EventKind::MoveStep, Some(EventSubject::Trip(trip_entity)));
        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("move step event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems(movement_system);
        schedule.run(&mut world);

        let driver_position = {
            let pos = world.query::<&Position>().get(&world, driver_entity).expect("pos");
            pos.0
        };
        assert_eq!(driver_position, neighbor);

        let eta_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("pickup eta updated event");
        assert_eq!(eta_event.kind, EventKind::PickupEtaUpdated);
        assert_eq!(eta_event.timestamp, 1000);
        assert_eq!(eta_event.subject, Some(EventSubject::Trip(trip_entity)));

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("trip started event");
        assert_eq!(next_event.kind, EventKind::TripStarted);
        assert_eq!(next_event.timestamp, 2000);
        assert_eq!(next_event.subject, Some(EventSubject::Trip(trip_entity)));
    }

    #[test]
    fn eta_ms_scales_with_distance() {
        let speed = 40.0;
        assert_eq!(travel_time_ms(0.0, speed), ONE_SEC_MS);
        assert_eq!(travel_time_ms(1.0, speed), 90_000);
        assert_eq!(travel_time_ms(2.5, speed), 225_000);
    }
}
