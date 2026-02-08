use bevy_ecs::prelude::{Commands, Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{
    Driver, DriverEarnings, DriverStateCommands, InTransit, OnTrip, Rider, RiderCompleted, Trip,
    TripCompleted, TripFinancials, TripOnTrip, TripTiming,
};
use crate::pricing::{
    calculate_driver_earnings, calculate_platform_revenue, calculate_trip_fare_with_config,
    PricingConfig,
};
use crate::telemetry::{CompletedTripRecord, SimTelemetry};

#[allow(clippy::too_many_arguments)]
pub fn trip_completed_system(
    event: Res<CurrentEvent>,
    mut clock: ResMut<SimulationClock>,
    pricing_config: Res<PricingConfig>,
    mut telemetry: ResMut<SimTelemetry>,
    mut commands: Commands,
    mut trips: Query<(
        &mut Trip,
        &mut TripTiming,
        &TripFinancials,
        Option<&TripOnTrip>,
    )>,
    mut riders: Query<(&mut Rider, Option<&InTransit>)>,
    mut drivers: Query<(&mut Driver, Option<&OnTrip>)>,
    mut driver_earnings: Query<&mut DriverEarnings>,
) {
    if event.0.kind != EventKind::TripCompleted {
        return;
    }

    let Some(EventSubject::Trip(trip_entity)) = event.0.subject else {
        return;
    };

    let Ok((trip, mut timing, financials, on_trip)) = trips.get_mut(trip_entity) else {
        return;
    };
    if on_trip.is_none() {
        return;
    }

    let driver_entity = trip.driver;
    let rider_entity = trip.rider;

    // Use agreed fare (quoted at accept, may include surge) or fall back to current pricing
    let fare = financials.agreed_fare.unwrap_or_else(|| {
        calculate_trip_fare_with_config(trip.pickup, trip.dropoff, *pricing_config)
    });

    // Calculate base fare (without surge) to determine surge impact
    let base_fare = calculate_trip_fare_with_config(trip.pickup, trip.dropoff, *pricing_config);
    let surge_impact = (fare - base_fare).max(0.0); // Ensure non-negative

    let commission = calculate_platform_revenue(fare, pricing_config.commission_rate);
    let driver_earnings_amount = calculate_driver_earnings(fare, pricing_config.commission_rate);

    // Update driver state and clear trip backlink
    if let Ok((mut driver, on_trip)) = drivers.get_mut(driver_entity) {
        if on_trip.is_some() {
            commands.entity(driver_entity).set_driver_state_idle();
        }
        driver.matched_rider = None;
        driver.assigned_trip = None;
    }

    // Update earnings
    if let Ok(mut earnings) = driver_earnings.get_mut(driver_entity) {
        earnings.daily_earnings += driver_earnings_amount;
    }

    // Delegate offduty threshold check to driver_offduty_check_system via targeted event
    clock.schedule_in(
        0,
        EventKind::CheckDriverOffDuty,
        Some(EventSubject::Driver(driver_entity)),
    );

    if let Ok((mut rider, in_transit)) = riders.get_mut(rider_entity) {
        if in_transit.is_some() {
            commands
                .entity(rider_entity)
                .remove::<InTransit>()
                .insert(RiderCompleted);
        }
        rider.matched_driver = None;
    }

    let completed_at = clock.now();
    let pickup_at = timing.pickup_at.unwrap_or(completed_at);
    timing.dropoff_at = Some(completed_at);
    commands
        .entity(trip_entity)
        .remove::<TripOnTrip>()
        .insert(TripCompleted);

    telemetry.completed_trips.push(CompletedTripRecord {
        trip_entity,
        rider_entity,
        driver_entity,
        completed_at,
        requested_at: timing.requested_at,
        matched_at: timing.matched_at,
        pickup_at,
        fare,
        surge_impact,
    });
    telemetry.riders_completed_total = telemetry.riders_completed_total.saturating_add(1);
    telemetry.platform_revenue_total += commission;
    telemetry.total_fares_collected += fare;

    commands.entity(rider_entity).despawn();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::SimulationClock;
    use crate::ecs::Idle;
    use bevy_ecs::prelude::{Schedule, World};
    use bevy_ecs::schedule::apply_deferred;

    #[test]
    fn trip_completed_transitions_driver_and_rider() {
        use crate::pricing::PricingConfig;

        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(crate::telemetry::SimTelemetry::default());
        world.insert_resource(PricingConfig::default());
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
                InTransit,
            ))
            .id();
        let driver_entity = world
            .spawn((
                Driver {
                    matched_rider: None,
                    assigned_trip: None,
                },
                OnTrip,
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
                TripOnTrip,
                TripTiming {
                    requested_at: 0,
                    matched_at: 1,
                    pickup_at: Some(2),
                    dropoff_at: None,
                    cancelled_at: None,
                },
                TripFinancials {
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
        {
            let mut driver_entity_mut = world.entity_mut(driver_entity);
            let mut driver = driver_entity_mut.get_mut::<Driver>().expect("driver");
            driver.matched_rider = Some(rider_entity);
        }

        world.resource_mut::<SimulationClock>().schedule_at_secs(
            2,
            EventKind::TripCompleted,
            Some(EventSubject::Trip(trip_entity)),
        );

        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("trip completed event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems((trip_completed_system, apply_deferred));
        schedule.run(&mut world);

        let rider_exists = world.query::<&Rider>().iter(&world).next().is_some();
        let (driver_state, matched_rider) = {
            let driver = world.query::<&Driver>().single(&world);
            (
                world.entity(driver_entity).contains::<Idle>(),
                driver.matched_rider,
            )
        };

        assert!(!rider_exists, "rider should be despawned on completion");
        assert!(driver_state);
        assert_eq!(matched_rider, None);

        assert!(world.entity(trip_entity).contains::<TripCompleted>());
    }
}
