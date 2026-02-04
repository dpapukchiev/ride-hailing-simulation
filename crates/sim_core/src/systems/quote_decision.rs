//! QuoteDecision system: rider stochastically accepts or rejects the shown quote.

use bevy_ecs::prelude::{Entity, Query, Res, ResMut};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Rider, RiderQuote, RiderState};
use crate::scenario::RiderQuoteConfig;
use crate::telemetry::RiderAbandonmentReason;

pub fn quote_decision_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    quote_config: Option<Res<RiderQuoteConfig>>,
    mut riders: Query<(Entity, &mut Rider, &RiderQuote)>,
) {
    if event.0.kind != EventKind::QuoteDecision {
        return;
    }

    let Some(EventSubject::Rider(rider_entity)) = event.0.subject else {
        return;
    };

    let Ok((_, mut rider, quote)) = riders.get_mut(rider_entity) else {
        return;
    };
    if rider.state != RiderState::Browsing {
        return;
    }

    let config = quote_config.as_deref().copied().unwrap_or_default();
    let over_price = quote.fare > config.max_willingness_to_pay;
    let over_eta = quote.eta_ms > config.max_acceptable_eta_ms;
    if over_price || over_eta {
        // Set rejection reason before scheduling rejection event
        rider.last_rejection_reason = Some(if over_price {
            RiderAbandonmentReason::QuotePriceTooHigh
        } else {
            RiderAbandonmentReason::QuoteEtaTooLong
        });
        clock.schedule_in_secs(
            0,
            EventKind::QuoteRejected,
            Some(EventSubject::Rider(rider_entity)),
        );
        return;
    }
    let seed = config.seed.wrapping_add(rider_entity.index() as u64);
    let mut rng = StdRng::seed_from_u64(seed);
    let accept = rng.gen::<f64>() < config.accept_probability;

    if accept {
        clock.schedule_in_secs(
            0,
            EventKind::QuoteAccepted,
            Some(EventSubject::Rider(rider_entity)),
        );
    } else {
        // Set rejection reason for stochastic rejection
        rider.last_rejection_reason = Some(RiderAbandonmentReason::QuoteStochasticRejection);
        clock.schedule_in_secs(
            0,
            EventKind::QuoteRejected,
            Some(EventSubject::Rider(rider_entity)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};
    use crate::ecs::Position;

    #[test]
    fn quote_decision_with_accept_probability_one_schedules_accepted() {
        use crate::test_helpers::test_neighbor_cell;

        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(RiderQuoteConfig {
            max_quote_rejections: 3,
            re_quote_delay_secs: 10,
            accept_probability: 1.0, // Always accept
            seed: 42,
            max_willingness_to_pay: 100.0,
            max_acceptable_eta_ms: 600_000,
        });
        let destination = test_neighbor_cell();
        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");

        let rider_entity = world
            .spawn((
                Rider {
                    state: RiderState::Browsing,
                    matched_driver: None,
                    destination: Some(destination),
                    requested_at: None,
                    quote_rejections: 0,
                    accepted_fare: None,
                    last_rejection_reason: None,
                },
                Position(cell),
                RiderQuote {
                    fare: 5.0,
                    eta_ms: 60_000,
                },
            ))
            .id();

        world.resource_mut::<SimulationClock>().schedule_at_secs(
            1,
            EventKind::QuoteDecision,
            Some(EventSubject::Rider(rider_entity)),
        );

        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("quote decision event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems(quote_decision_system);
        schedule.run(&mut world);

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("next event");
        assert_eq!(next_event.kind, EventKind::QuoteAccepted);
        assert_eq!(next_event.subject, Some(EventSubject::Rider(rider_entity)));
    }

    #[test]
    fn quote_decision_with_accept_probability_zero_schedules_rejected() {
        use crate::test_helpers::test_neighbor_cell;

        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(RiderQuoteConfig {
            max_quote_rejections: 3,
            re_quote_delay_secs: 10,
            accept_probability: 0.0, // Never accept
            seed: 42,
            max_willingness_to_pay: 100.0,
            max_acceptable_eta_ms: 600_000,
        });
        let destination = test_neighbor_cell();
        let cell = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");

        let rider_entity = world
            .spawn((
                Rider {
                    state: RiderState::Browsing,
                    matched_driver: None,
                    destination: Some(destination),
                    requested_at: None,
                    quote_rejections: 0,
                    accepted_fare: None,
                    last_rejection_reason: None,
                },
                Position(cell),
                RiderQuote {
                    fare: 5.0,
                    eta_ms: 60_000,
                },
            ))
            .id();

        world.resource_mut::<SimulationClock>().schedule_at_secs(
            1,
            EventKind::QuoteDecision,
            Some(EventSubject::Rider(rider_entity)),
        );

        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("quote decision event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems(quote_decision_system);
        schedule.run(&mut world);

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("next event");
        assert_eq!(next_event.kind, EventKind::QuoteRejected);
        assert_eq!(next_event.subject, Some(EventSubject::Rider(rider_entity)));
    }
}
