use bevy_ecs::prelude::{Commands, Entity, Query, Res, ResMut};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{
    Driver, DriverEarnings, DriverFatigue, DriverState, Position, Rider, RiderState, Trip,
    TripState,
};
use crate::scenario::{BatchMatchingConfig, DriverDecisionConfig};
use crate::spatial::distance_km_between_cells;

const MATCH_RETRY_SECS: u64 = 30;

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
    batch_config: Option<Res<BatchMatchingConfig>>,
    driver_config: Option<Res<DriverDecisionConfig>>,
    mut commands: Commands,
    mut drivers: Query<(&mut Driver, &Position, &DriverEarnings, &DriverFatigue)>,
    mut riders: Query<(Entity, &mut Rider, &Position)>,
) {
    if event.0.kind != EventKind::DriverDecision {
        return;
    }

    let Some(EventSubject::Driver(driver_entity)) = event.0.subject else {
        return;
    };
    let Ok((mut driver, driver_pos, driver_earnings, driver_fatigue)) =
        drivers.get_mut(driver_entity)
    else {
        return;
    };
    if driver.state != DriverState::Evaluating {
        return;
    }

    let Some(rider_entity) = driver.matched_rider else {
        driver.state = DriverState::Idle;
        return;
    };

    // Get trip characteristics for score calculation
    let (pickup, dropoff, requested_at, rider_waiting, fare) = match riders.get_mut(rider_entity) {
        Ok((_entity, rider, pos)) => {
            let pickup = pos.0;
            let Some(dropoff) = rider.destination else {
                // Rider has no destination; reset driver and bail
                driver.state = DriverState::Idle;
                driver.matched_rider = None;
                return;
            };
            let requested_at = rider.requested_at.unwrap_or(clock.now());
            let fare = rider.accepted_fare.unwrap_or(0.0);
            (
                pickup,
                dropoff,
                requested_at,
                rider.state == RiderState::Waiting,
                fare,
            )
        }
        Err(_) => {
            driver.state = DriverState::Idle;
            return;
        }
    };

    if !rider_waiting {
        driver.state = DriverState::Idle;
        driver.matched_rider = None;
        if let Ok((_entity, mut rider, _)) = riders.get_mut(rider_entity) {
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
        driver.state = DriverState::EnRoute;
        let agreed_fare = riders
            .get(rider_entity)
            .ok()
            .and_then(|(_, r, _)| r.accepted_fare);

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
                agreed_fare,
            })
            .id();

        clock.schedule_in_secs(
            1,
            EventKind::MoveStep,
            Some(EventSubject::Trip(trip_entity)),
        );
    } else {
        let rejected_rider = driver.matched_rider;
        driver.state = DriverState::Idle;
        driver.matched_rider = None;
        if let Some(rider_entity) = rejected_rider {
            if let Ok((_entity, mut rider, _)) = riders.get_mut(rider_entity) {
                rider.matched_driver = None;
                // Only schedule per-rider TryMatch when batch matching is disabled
                let batch_enabled = batch_config.as_deref().is_some_and(|c| c.enabled);
                if rider.state == RiderState::Waiting && !batch_enabled {
                    clock.schedule_in_secs(
                        MATCH_RETRY_SECS,
                        EventKind::TryMatch,
                        Some(EventSubject::Rider(rider_entity)),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};
    use bevy_ecs::schedule::apply_deferred;

    use crate::ecs::{DriverEarnings, DriverFatigue, Position, Rider, RiderState};

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
                    state: RiderState::Waiting,
                    matched_driver: None,
                    destination: Some(destination),
                    requested_at: None,
                    quote_rejections: 0,
                    accepted_fare: Some(15.0),
                    last_rejection_reason: None,
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
                    state: RiderState::Waiting,
                    matched_driver: None,
                    destination: Some(destination),
                    requested_at: None,
                    quote_rejections: 0,
                    accepted_fare: Some(5.0), // Low fare
                    last_rejection_reason: None,
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
        assert_eq!(driver.state, DriverState::Idle);
        assert_eq!(driver.matched_rider, None);

        let rider = world.query::<&Rider>().single(&world);
        assert_eq!(rider.matched_driver, None);
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
                    state: RiderState::Waiting,
                    matched_driver: None,
                    destination: Some(destination),
                    requested_at: None,
                    quote_rejections: 0,
                    accepted_fare: Some(10.0),
                    last_rejection_reason: None,
                },
                Position(cell),
            ))
            .id();
        let driver_entity1 = world1
            .spawn((
                Driver {
                    state: DriverState::Evaluating,
                    matched_rider: Some(rider_entity1),
                },
                Position(cell),
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
                    state: RiderState::Waiting,
                    matched_driver: None,
                    destination: Some(destination),
                    requested_at: None,
                    quote_rejections: 0,
                    accepted_fare: Some(10.0),
                    last_rejection_reason: None,
                },
                Position(cell),
            ))
            .id();
        let driver_entity2 = world2
            .spawn((
                Driver {
                    state: DriverState::Evaluating,
                    matched_rider: Some(rider_entity2),
                },
                Position(cell),
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

        let driver1 = world1.query::<&Driver>().single(&world1);
        let driver2 = world2.query::<&Driver>().single(&world2);

        // Same seed + same driver entity index = same decision
        assert_eq!(driver1.state, driver2.state);
    }
}
