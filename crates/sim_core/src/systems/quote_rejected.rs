//! QuoteRejected system: rider rejected the quote; retry with new quote or give up.

use bevy_ecs::prelude::{Commands, Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Browsing, Rider};
use crate::scenario::RiderQuoteConfig;
use crate::telemetry::{RiderAbandonmentReason, SimTelemetry};

pub fn quote_rejected_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    mut commands: Commands,
    mut telemetry: ResMut<SimTelemetry>,
    quote_config: Option<Res<RiderQuoteConfig>>,
    mut riders: Query<(&mut Rider, Option<&Browsing>)>,
) {
    if event.0.kind != EventKind::QuoteRejected {
        return;
    }

    let Some(EventSubject::Rider(rider_entity)) = event.0.subject else {
        return;
    };

    let Ok((mut rider, browsing)) = riders.get_mut(rider_entity) else {
        return;
    };
    if browsing.is_none() {
        return;
    }

    rider.quote_rejections += 1;
    let config = quote_config.as_deref().copied().unwrap_or_default();

    if rider.quote_rejections <= config.max_quote_rejections {
        clock.schedule_in_secs(
            config.re_quote_delay_secs,
            EventKind::ShowQuote,
            Some(EventSubject::Rider(rider_entity)),
        );
    } else {
        // Rider gives up - record abandonment reason
        telemetry.riders_abandoned_quote_total =
            telemetry.riders_abandoned_quote_total.saturating_add(1);

        // Track breakdown by reason
        match rider.last_rejection_reason {
            Some(RiderAbandonmentReason::QuotePriceTooHigh) => {
                telemetry.riders_abandoned_price =
                    telemetry.riders_abandoned_price.saturating_add(1);
            }
            Some(RiderAbandonmentReason::QuoteEtaTooLong) => {
                telemetry.riders_abandoned_eta = telemetry.riders_abandoned_eta.saturating_add(1);
            }
            Some(RiderAbandonmentReason::QuoteStochasticRejection) => {
                telemetry.riders_abandoned_stochastic =
                    telemetry.riders_abandoned_stochastic.saturating_add(1);
            }
            _ => {
                // Fallback: if no reason recorded, count as stochastic (shouldn't happen in normal flow)
                telemetry.riders_abandoned_stochastic =
                    telemetry.riders_abandoned_stochastic.saturating_add(1);
            }
        }

        commands.entity(rider_entity).despawn();
    }
}
