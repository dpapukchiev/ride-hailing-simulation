use bevy_ecs::prelude::{Commands, Query, Res, ResMut};
use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Driver, DriverState, Position, Rider, RiderState, Trip, TripState};

fn logit_accepts(score: f64) -> bool {
    let probability = 1.0 / (1.0 + (-score).exp());
    probability >= 0.5
}

fn default_dropoff(pickup: h3o::CellIndex) -> h3o::CellIndex {
    pickup
        .grid_disk::<Vec<_>>(1)
        .into_iter()
        .find(|c| *c != pickup)
        .unwrap_or(pickup)
}

pub fn driver_decision_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    mut commands: Commands,
    mut drivers: Query<(&mut Driver, &Position)>,
    riders: Query<(&Rider, &Position)>,
) {
    if event.0.kind != EventKind::DriverDecision {
        return;
    }

    let Some(EventSubject::Driver(driver_entity)) = event.0.subject else {
        return;
    };
    let Ok((mut driver, driver_pos)) = drivers.get_mut(driver_entity) else {
        return;
    };
    if driver.state != DriverState::Evaluating {
        return;
    }

    if logit_accepts(1.0) {
        let Some(rider_entity) = driver.matched_rider else {
            driver.state = DriverState::Idle;
            return;
        };

        let (pickup, dropoff, requested_at, rider_waiting) = match riders.get(rider_entity) {
            Ok((rider, pos)) => {
                let pickup = pos.0;
                let dropoff = rider
                    .destination
                    .unwrap_or_else(|| default_dropoff(pickup));
                let requested_at = rider.requested_at.unwrap_or(clock.now());
                (pickup, dropoff, requested_at, rider.state == RiderState::Waiting)
            }
            Err(_) => {
                driver.state = DriverState::Idle;
                return;
            }
        };
        if !rider_waiting {
            driver.state = DriverState::Idle;
            driver.matched_rider = None;
            return;
        }

        let matched_at = clock.now();
        let pickup_distance_km_at_accept = distance_km_between_cells(driver_pos.0, pickup);
        driver.state = DriverState::EnRoute;
        let trip_entity = commands
            .spawn(Trip {
                state: TripState::EnRoute,
                rider: rider_entity,
                driver: driver_entity,
                pickup,
                dropoff,
                pickup_distance_km_at_accept,
                requested_at,
                matched_at,
                pickup_at: None,
                pickup_eta_ms: 0,
                dropoff_at: None,
                cancelled_at: None,
            })
            .id();

        clock.schedule_in_secs(1, EventKind::MoveStep, Some(EventSubject::Trip(trip_entity)));
    } else {
        driver.state = DriverState::Idle;
        driver.matched_rider = None;
    }
}

fn distance_km_between_cells(a: h3o::CellIndex, b: h3o::CellIndex) -> f64 {
    let a: h3o::LatLng = a.into();
    let b: h3o::LatLng = b.into();
    let (lat1, lon1) = (a.lat().to_radians(), a.lng().to_radians());
    let (lat2, lon2) = (b.lat().to_radians(), b.lng().to_radians());
    let dlat = lat2 - lat1;
    let dlon = lon2 - lon1;
    let sin_dlat = (dlat * 0.5).sin();
    let sin_dlon = (dlon * 0.5).sin();
    let h = sin_dlat * sin_dlat + lat1.cos() * lat2.cos() * sin_dlon * sin_dlon;
    let c = 2.0 * h.sqrt().atan2((1.0 - h).sqrt());
    6371.0 * c
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::apply_deferred;
    use bevy_ecs::prelude::{Schedule, World};

    use crate::ecs::{Position, Rider, RiderState};

    #[test]
    fn evaluating_driver_moves_to_en_route() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
        let rider_entity = world
            .spawn((
                Rider {
                    state: RiderState::Waiting,
                    matched_driver: None,
                    destination: None,
                    requested_at: None,
                },
                Position(cell),
            ))
            .id();
        let driver_entity = world
            .spawn((
                Driver {
                    state: DriverState::Evaluating,
                    matched_rider: Some(rider_entity),
                },
                Position(cell),
            ))
            .id();
        world
            .resource_mut::<SimulationClock>()
            .schedule_at_secs(1, EventKind::DriverDecision, Some(EventSubject::Driver(driver_entity)));

        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("driver decision event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems((driver_decision_system, apply_deferred));
        schedule.run(&mut world);

        let driver = world.query::<&Driver>().single(&world);
        assert_eq!(driver.state, DriverState::EnRoute);

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("move step event");
        assert_eq!(next_event.kind, EventKind::MoveStep);
        assert_eq!(next_event.timestamp, 2000);
        let trip_entity = match next_event.subject {
            Some(EventSubject::Trip(trip_entity)) => trip_entity,
            other => panic!("expected trip subject, got {other:?}"),
        };
        let trip = world.entity(trip_entity).get::<Trip>().expect("trip");
        assert_eq!(trip.state, TripState::EnRoute);
        assert_eq!(trip.driver, driver_entity);
        assert_eq!(trip.rider, rider_entity);
        assert_eq!(trip.pickup, cell);
        // dropoff is a neighbor of pickup when destination is None
    }
}
