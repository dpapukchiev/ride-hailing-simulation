use bevy_ecs::prelude::{Commands, ParamSet, Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{
    Driver, DriverStateCommands, EnRoute, InTransit, Position, Rider, Trip, TripEnRoute,
    TripOnTrip, TripRoute, TripTiming, Waiting,
};

#[allow(clippy::type_complexity)]
pub fn trip_started_system(
    mut commands: Commands,
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    mut trips: Query<(&mut Trip, &mut TripTiming, Option<&TripEnRoute>)>,
    mut queries: ParamSet<(
        Query<(&mut Driver, &Position, Option<&EnRoute>)>,
        Query<(&mut Rider, &mut Position, Option<&Waiting>)>,
    )>,
) {
    if event.0.kind != EventKind::TripStarted {
        return;
    }

    let Some(EventSubject::Trip(trip_entity)) = event.0.subject else {
        return;
    };

    let (driver_entity, rider_entity) = {
        let Ok((trip, _, en_route)) = trips.get(trip_entity) else {
            return;
        };
        if en_route.is_none() {
            return;
        }
        (trip.driver, trip.rider)
    };

    let driver_pos = {
        let driver_query = queries.p0();
        let Ok((_driver, driver_pos, en_route)) = driver_query.get(driver_entity) else {
            return;
        };
        if en_route.is_none() {
            return;
        }
        driver_pos.0
    };

    let (rider_pos, rider_matched_driver_ok, rider_waiting) = {
        let rider_query = queries.p1();
        let Ok((rider, rider_pos, waiting)) = rider_query.get(rider_entity) else {
            return;
        };
        (
            rider_pos.0,
            rider.matched_driver == Some(driver_entity),
            waiting.is_some(),
        )
    };
    if !rider_matched_driver_ok || !rider_waiting || rider_pos != driver_pos {
        return;
    }

    // Update rider state and position
    {
        let mut rider_query = queries.p1();
        let Ok((_rider, mut rider_pos, _)) = rider_query.get_mut(rider_entity) else {
            return;
        };
        commands
            .entity(rider_entity)
            .remove::<Waiting>()
            .insert(InTransit);
        // Update rider position to match driver position (rider is now in the vehicle)
        rider_pos.0 = driver_pos;
    }

    // Update driver state
    {
        let mut driver_query = queries.p0();
        let Ok((_driver, _, _)) = driver_query.get_mut(driver_entity) else {
            return;
        };
        commands.entity(driver_entity).set_driver_state_on_trip();
    }
    if let Ok((mut _trip, mut timing, _)) = trips.get_mut(trip_entity) {
        commands
            .entity(trip_entity)
            .remove::<TripEnRoute>()
            .insert(TripOnTrip);
        timing.pickup_at = Some(clock.now());
    }

    // Remove the pickup-leg TripRoute so the movement system will resolve a
    // fresh route for the dropoff leg on the next MoveStep.
    commands.entity(trip_entity).remove::<TripRoute>();

    // Start moving driver toward dropoff; completion is scheduled by movement when driver arrives.
    clock.schedule_in_secs(
        1,
        EventKind::MoveStep,
        Some(EventSubject::Trip(trip_entity)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::{GeoPosition, OnTrip};
    use bevy_ecs::prelude::{Schedule, World};
    use bevy_ecs::schedule::apply_deferred;

    #[test]
    fn trip_started_transitions_and_schedules_completion() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
        let destination = cell
            .grid_disk::<Vec<_>>(1)
            .into_iter()
            .find(|c| *c != cell)
            .expect("neighbor cell");

        let rider_entity = world
            .spawn((
                Rider {
                    matched_driver: None,
                    assigned_trip: None,
                    destination: Some(destination),
                    requested_at: None,
                    quote_rejections: 0,
                    accepted_fare: None,
                    last_rejection_reason: None,
                },
                Waiting,
                Position(cell),
                GeoPosition(cell.into()),
            ))
            .id();
        let driver_entity = world
            .spawn((
                Driver {
                    matched_rider: Some(rider_entity),
                    assigned_trip: None,
                },
                EnRoute,
                Position(cell),
                GeoPosition(cell.into()),
            ))
            .id();
        let trip_entity = world
            .spawn((
                Trip {
                    rider: rider_entity,
                    driver: driver_entity,
                    pickup: cell,
                    dropoff: destination,
                },
                TripEnRoute,
                TripTiming {
                    requested_at: 0,
                    matched_at: 0,
                    pickup_at: None,
                    dropoff_at: None,
                    cancelled_at: None,
                },
                crate::ecs::TripFinancials {
                    agreed_fare: None,
                    pickup_distance_km_at_accept: 0.0,
                },
                crate::ecs::TripLiveData { pickup_eta_ms: 0 },
            ))
            .id();

        {
            let mut rider_entity_mut = world.entity_mut(rider_entity);
            let mut rider = rider_entity_mut.get_mut::<Rider>().expect("rider");
            rider.matched_driver = Some(driver_entity);
        }

        world.resource_mut::<SimulationClock>().schedule_at_secs(
            3,
            EventKind::TripStarted,
            Some(EventSubject::Trip(trip_entity)),
        );

        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("trip started event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems((trip_started_system, apply_deferred));
        schedule.run(&mut world);

        assert!(world.entity(rider_entity).contains::<InTransit>());
        assert!(world.entity(driver_entity).contains::<OnTrip>());
        assert!(world.entity(trip_entity).contains::<TripOnTrip>());

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("move step toward dropoff");
        assert_eq!(next_event.kind, EventKind::MoveStep);
        assert_eq!(next_event.timestamp, 4000);
        assert_eq!(next_event.subject, Some(EventSubject::Trip(trip_entity)));
    }
}
