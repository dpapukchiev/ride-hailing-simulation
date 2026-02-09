//! QuoteDecision system: rider stochastically accepts or rejects the shown quote.

use bevy_ecs::prelude::{Entity, Query, Res, ResMut};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Browsing, Rider, RiderQuote};
use crate::scenario::RiderQuoteConfig;
use crate::telemetry::RiderAbandonmentReason;

pub fn quote_decision_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    quote_config: Option<Res<RiderQuoteConfig>>,
    mut riders: Query<(Entity, &mut Rider, &RiderQuote, Option<&Browsing>)>,
) {
    if event.0.kind != EventKind::QuoteDecision {
        return;
    }

    let Some(EventSubject::Rider(rider_entity)) = event.0.subject else {
        return;
    };

    let Ok((_, mut rider, quote, browsing)) = riders.get_mut(rider_entity) else {
        return;
    };
    if browsing.is_none() {
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
