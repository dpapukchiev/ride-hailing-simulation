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
