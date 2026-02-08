use bevy_ecs::prelude::{Commands, Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Driver, DriverState, Rider, RiderState, Trip, TripState, TripTiming};
use crate::telemetry::SimTelemetry;

pub fn rider_cancel_system(
    event: Res<CurrentEvent>,
    clock: Res<SimulationClock>,
    mut commands: Commands,
    mut telemetry: ResMut<SimTelemetry>,
    mut riders: Query<&mut Rider>,
    mut drivers: Query<&mut Driver>,
    mut trips: Query<(&mut Trip, &mut TripTiming)>,
) {
    if event.0.kind != EventKind::RiderCancel {
        return;
    }

    let Some(EventSubject::Rider(rider_entity)) = event.0.subject else {
        return;
    };
    let Ok(mut rider) = riders.get_mut(rider_entity) else {
        return;
    };
    if rider.state != RiderState::Waiting {
        return;
    }

    if let Some(driver_entity) = rider.matched_driver {
        // Use assigned_trip for O(1) trip lookup instead of scanning all trips
        if let Some(trip_entity) = rider.assigned_trip {
            if let Ok((mut trip, mut timing)) = trips.get_mut(trip_entity) {
                if trip.state == TripState::EnRoute {
                    trip.state = TripState::Cancelled;
                    timing.cancelled_at = Some(clock.now());
                }
            }
        }

        if let Ok(mut driver) = drivers.get_mut(driver_entity) {
            if driver.matched_rider == Some(rider_entity) {
                if driver.state == DriverState::EnRoute || driver.state == DriverState::Evaluating {
                    driver.state = DriverState::Idle;
                }
                driver.matched_rider = None;
            }
            // Clear the trip backlink from the driver
            driver.assigned_trip = None;
        }
    }

    rider.state = RiderState::Cancelled;
    rider.matched_driver = None;
    rider.assigned_trip = None;
    telemetry.riders_cancelled_total = telemetry.riders_cancelled_total.saturating_add(1);
    // Track pickup timeout cancellation
    telemetry.riders_cancelled_pickup_timeout =
        telemetry.riders_cancelled_pickup_timeout.saturating_add(1);
    commands.entity(rider_entity).despawn();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};
    use bevy_ecs::schedule::apply_deferred;

    use crate::clock::SimulationClock;
    use crate::ecs::{Position, Rider, Trip, TripFinancials, TripLiveData, TripTiming};

    #[test]
    fn rider_cancel_resets_driver_and_trip() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(SimTelemetry::default());
        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
        let destination = cell
            .grid_disk::<Vec<_>>(1)
            .into_iter()
            .find(|c| *c != cell)
            .expect("neighbor cell");

        let rider_entity = world
            .spawn((
                Rider {
                    state: RiderState::Waiting,
                    matched_driver: None,
                    assigned_trip: None,
                    destination: Some(destination),
                    requested_at: None,
                    quote_rejections: 0,
                    accepted_fare: None,
                    last_rejection_reason: None,
                },
                Position(cell),
            ))
            .id();
        let driver_entity = world
            .spawn((
                Driver {
                    state: DriverState::EnRoute,
                    matched_rider: Some(rider_entity),
                    assigned_trip: None,
                },
                Position(cell),
            ))
            .id();
        let trip_entity = world
            .spawn((
                Trip {
                    state: TripState::EnRoute,
                    rider: rider_entity,
                    driver: driver_entity,
                    pickup: cell,
                    dropoff: destination,
                },
                TripTiming {
                    requested_at: 0,
                    matched_at: 0,
                    pickup_at: None,
                    dropoff_at: None,
                    cancelled_at: None,
                },
                TripFinancials {
                    agreed_fare: None,
                    pickup_distance_km_at_accept: 0.0,
                },
                TripLiveData { pickup_eta_ms: 0 },
            ))
            .id();

        {
            let mut rider_entity_mut = world.entity_mut(rider_entity);
            let mut rider = rider_entity_mut.get_mut::<Rider>().expect("rider");
            rider.matched_driver = Some(driver_entity);
            rider.assigned_trip = Some(trip_entity);
        }
        {
            let mut driver_entity_mut = world.entity_mut(driver_entity);
            let mut driver = driver_entity_mut.get_mut::<Driver>().expect("driver");
            driver.assigned_trip = Some(trip_entity);
        }

        world.resource_mut::<SimulationClock>().schedule_at_secs(
            1,
            EventKind::RiderCancel,
            Some(EventSubject::Rider(rider_entity)),
        );
        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("rider cancel event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems((rider_cancel_system, apply_deferred));
        schedule.run(&mut world);

        let rider_exists = world.get_entity(rider_entity).is_some();
        assert!(!rider_exists, "rider should be despawned on cancel");

        let driver = world.entity(driver_entity).get::<Driver>().expect("driver");
        assert_eq!(driver.state, DriverState::Idle);
        assert_eq!(driver.matched_rider, None);

        let trip = world.entity(trip_entity).get::<Trip>().expect("trip");
        assert_eq!(trip.state, TripState::Cancelled);
    }

    #[test]
    fn rider_cancel_without_match_despawns_rider() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(SimTelemetry::default());
        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
        let destination = cell
            .grid_disk::<Vec<_>>(1)
            .into_iter()
            .find(|c| *c != cell)
            .expect("neighbor cell");

        let rider_entity = world
            .spawn((
                Rider {
                    state: RiderState::Waiting,
                    matched_driver: None,
                    assigned_trip: None,
                    destination: Some(destination),
                    requested_at: None,
                    quote_rejections: 0,
                    accepted_fare: None,
                    last_rejection_reason: None,
                },
                Position(cell),
            ))
            .id();

        world.resource_mut::<SimulationClock>().schedule_at_secs(
            1,
            EventKind::RiderCancel,
            Some(EventSubject::Rider(rider_entity)),
        );
        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("rider cancel event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems((rider_cancel_system, apply_deferred));
        schedule.run(&mut world);

        let rider_exists = world.get_entity(rider_entity).is_some();
        assert!(!rider_exists, "rider should be despawned on cancel");
    }
}
