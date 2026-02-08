use bevy_ecs::prelude::{Commands, Entity, Query, Res, ResMut, With};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{
    Driver, DriverEarnings, DriverFatigue, DriverStateCommands, Evaluating, Position, Rider, Trip,
    TripEnRoute, TripFinancials, TripLiveData, TripTiming, Waiting,
};
use crate::scenario::DriverDecisionConfig;
use crate::spatial::distance_km_between_cells;

/// Calculate logit probability from score and sample stochastically using seeded RNG.
fn logit_accepts_stochastic(score: f64, seed: u64, driver_entity: Entity) -> bool {
    let probability = 1.0 / (1.0 + (-score).exp());
    let rng_seed = seed.wrapping_add(driver_entity.index() as u64);
    let mut rng = StdRng::seed_from_u64(rng_seed);
    rng.gen::<f64>() < probability
}

pub fn driver_decision_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    driver_config: Option<Res<DriverDecisionConfig>>,
    mut commands: Commands,
    mut drivers: Query<
        (
            Entity,
            &mut Driver,
            &Position,
            &DriverEarnings,
            &DriverFatigue,
        ),
        With<Evaluating>,
    >,
    mut riders: Query<(Entity, &mut Rider, &Position, Option<&Waiting>)>,
) {
    if event.0.kind != EventKind::DriverDecision {
        return;
    }

    let Some(EventSubject::Driver(driver_entity)) = event.0.subject else {
        return;
    };
    let Ok((driver_entity, mut driver, driver_pos, driver_earnings, driver_fatigue)) =
        drivers.get_mut(driver_entity)
    else {
        return;
    };

    let Some(rider_entity) = driver.matched_rider else {
        commands.entity(driver_entity).set_driver_state_idle();
        return;
    };

    // Get trip characteristics for score calculation
    let (pickup, dropoff, requested_at, rider_waiting, fare) = match riders.get_mut(rider_entity) {
        Ok((_entity, rider, pos, waiting)) => {
            let pickup = pos.0;
            let Some(dropoff) = rider.destination else {
                // Rider has no destination; reset driver and bail
                commands.entity(driver_entity).set_driver_state_idle();
                driver.matched_rider = None;
                return;
            };
            let requested_at = rider.requested_at.unwrap_or(clock.now());
            let fare = rider.accepted_fare.unwrap_or(0.0);
            (pickup, dropoff, requested_at, waiting.is_some(), fare)
        }
        Err(_) => {
            commands.entity(driver_entity).set_driver_state_idle();
            return;
        }
    };

    if !rider_waiting {
        commands.entity(driver_entity).set_driver_state_idle();
        driver.matched_rider = None;
        if let Ok((_entity, mut rider, _, _)) = riders.get_mut(rider_entity) {
            rider.matched_driver = None;
        }
        return;
    }

    // Calculate logit score based on trip and driver characteristics
    let config = driver_config.as_deref().copied().unwrap_or_default();

    let pickup_distance_km = distance_km_between_cells(driver_pos.0, pickup);
    let trip_distance_km = distance_km_between_cells(pickup, dropoff);

    // Get driver state metrics
    let earnings_progress =
        driver_earnings.daily_earnings / driver_earnings.daily_earnings_target.max(1.0);
    let session_duration_ms = clock
        .now()
        .saturating_sub(driver_earnings.session_start_time_ms);
    let fatigue_ratio =
        session_duration_ms as f64 / driver_fatigue.fatigue_threshold_ms.max(1) as f64;

    // Calculate score: higher score = higher acceptance probability
    let score = config.base_acceptance_score
        + (fare * config.fare_weight)
        + (pickup_distance_km * config.pickup_distance_penalty)
        + (trip_distance_km * config.trip_distance_bonus)
        + (earnings_progress * config.earnings_progress_weight)
        + (fatigue_ratio * config.fatigue_penalty);

    if logit_accepts_stochastic(score, config.seed, driver_entity) {
        let matched_at = clock.now();
        let pickup_distance_km_at_accept = distance_km_between_cells(driver_pos.0, pickup);
        commands.entity(driver_entity).set_driver_state_en_route();
        let agreed_fare = riders
            .get(rider_entity)
            .ok()
            .and_then(|(_, r, _, _)| r.accepted_fare);

        let trip_entity = commands
            .spawn((
                Trip {
                    rider: rider_entity,
                    driver: driver_entity,
                    pickup,
                    dropoff,
                },
                TripEnRoute,
                TripTiming {
                    requested_at,
                    matched_at,
                    pickup_at: None,
                    dropoff_at: None,
                    cancelled_at: None,
                },
                TripFinancials {
                    agreed_fare,
                    pickup_distance_km_at_accept,
                },
                TripLiveData { pickup_eta_ms: 0 },
            ))
            .id();

        // Set trip backlinks on rider and driver for O(1) lookup
        if let Ok((_entity, mut rider, _pos, _waiting)) = riders.get_mut(rider_entity) {
            rider.assigned_trip = Some(trip_entity);
        }
        driver.assigned_trip = Some(trip_entity);

        clock.schedule_in_secs(
            1,
            EventKind::MoveStep,
            Some(EventSubject::Trip(trip_entity)),
        );
    } else {
        let rejected_rider = driver.matched_rider;
        commands.entity(driver_entity).set_driver_state_idle();
        driver.matched_rider = None;
        if let Some(rider_entity) = rejected_rider {
            // Delegate rider-side cleanup to match_rejected_system
            clock.schedule_in(
                0,
                EventKind::MatchRejected,
                Some(EventSubject::Rider(rider_entity)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};
    use bevy_ecs::schedule::apply_deferred;

    use crate::ecs::{
        DriverEarnings, DriverFatigue, EnRoute, Evaluating, GeoPosition, Idle, Position, Rider,
        TripEnRoute, Waiting,
    };

    #[test]
    fn evaluating_driver_moves_to_en_route() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(DriverDecisionConfig {
            seed: 42,
            base_acceptance_score: 10.0, // High score to ensure acceptance
            ..Default::default()
        });
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
                    accepted_fare: Some(15.0),
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
                Evaluating,
                Position(cell),
                GeoPosition(cell.into()),
                DriverEarnings {
                    daily_earnings: 0.0,
                    daily_earnings_target: 200.0,
                    session_start_time_ms: 0,
                    session_end_time_ms: None,
                },
                DriverFatigue {
                    fatigue_threshold_ms: 8 * 3600 * 1000, // 8 hours
                },
            ))
            .id();
        world.resource_mut::<SimulationClock>().schedule_at_secs(
            1,
            EventKind::DriverDecision,
            Some(EventSubject::Driver(driver_entity)),
        );

        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("driver decision event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems((driver_decision_system, apply_deferred));
        schedule.run(&mut world);

        assert!(world.entity(driver_entity).contains::<EnRoute>());

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
        assert!(world.entity(trip_entity).contains::<TripEnRoute>());
        assert_eq!(trip.driver, driver_entity);
        assert_eq!(trip.rider, rider_entity);
        assert_eq!(trip.pickup, cell);
        assert_eq!(trip.dropoff, destination);
    }

    #[test]
    fn driver_rejects_with_very_negative_score() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(DriverDecisionConfig {
            seed: 42,
            base_acceptance_score: -100.0, // Very negative score to ensure rejection
            ..Default::default()
        });
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
                    accepted_fare: Some(5.0), // Low fare
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
                Evaluating,
                Position(cell),
                GeoPosition(cell.into()),
                DriverEarnings {
                    daily_earnings: 0.0,
                    daily_earnings_target: 200.0,
                    session_start_time_ms: 0,
                    session_end_time_ms: None,
                },
                DriverFatigue {
                    fatigue_threshold_ms: 8 * 3600 * 1000,
                },
            ))
            .id();
        world.resource_mut::<SimulationClock>().schedule_at_secs(
            1,
            EventKind::DriverDecision,
            Some(EventSubject::Driver(driver_entity)),
        );

        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("driver decision event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems((driver_decision_system, apply_deferred));
        schedule.run(&mut world);

        let driver = world.query::<&Driver>().single(&world);
        assert!(world.entity(driver_entity).contains::<Idle>());
        assert_eq!(driver.matched_rider, None);

        // MatchRejected event should be scheduled at delta 0 for the rider
        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("match rejected event");
        assert_eq!(next_event.kind, EventKind::MatchRejected);
        assert_eq!(next_event.timestamp, 1000); // delta 0 from DriverDecision at 1s
        assert_eq!(next_event.subject, Some(EventSubject::Rider(rider_entity)));
    }

    #[test]
    fn driver_decision_is_reproducible_with_same_seed() {
        let mut world1 = World::new();
        let mut world2 = World::new();

        for world in [&mut world1, &mut world2] {
            world.insert_resource(SimulationClock::default());
            world.insert_resource(DriverDecisionConfig {
                seed: 12345,
                base_acceptance_score: 0.0, // Moderate score
                ..Default::default()
            });
        }

        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
        let destination = cell
            .grid_disk::<Vec<_>>(1)
            .into_iter()
            .find(|c| *c != cell)
            .expect("neighbor cell");

        let rider_entity1 = world1
            .spawn((
                Rider {
                    matched_driver: None,
                    assigned_trip: None,
                    destination: Some(destination),
                    requested_at: None,
                    quote_rejections: 0,
                    accepted_fare: Some(10.0),
                    last_rejection_reason: None,
                },
                Waiting,
                Position(cell),
                GeoPosition(cell.into()),
            ))
            .id();
        let driver_entity1 = world1
            .spawn((
                Driver {
                    matched_rider: Some(rider_entity1),
                    assigned_trip: None,
                },
                Evaluating,
                Position(cell),
                GeoPosition(cell.into()),
                DriverEarnings {
                    daily_earnings: 0.0,
                    daily_earnings_target: 200.0,
                    session_start_time_ms: 0,
                    session_end_time_ms: None,
                },
                DriverFatigue {
                    fatigue_threshold_ms: 8 * 3600 * 1000,
                },
            ))
            .id();

        let rider_entity2 = world2
            .spawn((
                Rider {
                    matched_driver: None,
                    assigned_trip: None,
                    destination: Some(destination),
                    requested_at: None,
                    quote_rejections: 0,
                    accepted_fare: Some(10.0),
                    last_rejection_reason: None,
                },
                Waiting,
                Position(cell),
                GeoPosition(cell.into()),
            ))
            .id();
        let driver_entity2 = world2
            .spawn((
                Driver {
                    matched_rider: Some(rider_entity2),
                    assigned_trip: None,
                },
                Evaluating,
                Position(cell),
                GeoPosition(cell.into()),
                DriverEarnings {
                    daily_earnings: 0.0,
                    daily_earnings_target: 200.0,
                    session_start_time_ms: 0,
                    session_end_time_ms: None,
                },
                DriverFatigue {
                    fatigue_threshold_ms: 8 * 3600 * 1000,
                },
            ))
            .id();

        // Ensure driver entities have same index for reproducibility
        assert_eq!(driver_entity1.index(), driver_entity2.index());

        for (world, driver_entity) in [(&mut world1, driver_entity1), (&mut world2, driver_entity2)]
        {
            world.resource_mut::<SimulationClock>().schedule_at_secs(
                1,
                EventKind::DriverDecision,
                Some(EventSubject::Driver(driver_entity)),
            );

            let event = world
                .resource_mut::<SimulationClock>()
                .pop_next()
                .expect("driver decision event");
            world.insert_resource(CurrentEvent(event));

            let mut schedule = Schedule::default();
            schedule.add_systems((driver_decision_system, apply_deferred));
            schedule.run(world);
        }

        // Same seed + same driver entity index = same decision
        assert_eq!(
            world1.entity(driver_entity1).contains::<EnRoute>(),
            world2.entity(driver_entity2).contains::<EnRoute>()
        );
    }
}
