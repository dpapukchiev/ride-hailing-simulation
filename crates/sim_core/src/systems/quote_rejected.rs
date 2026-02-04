//! QuoteRejected system: rider rejected the quote; retry with new quote or give up.

use bevy_ecs::prelude::{Commands, Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Rider, RiderState};
use crate::scenario::RiderQuoteConfig;
use crate::telemetry::{RiderAbandonmentReason, SimTelemetry};

pub fn quote_rejected_system(
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    mut commands: Commands,
    mut telemetry: ResMut<SimTelemetry>,
    quote_config: Option<Res<RiderQuoteConfig>>,
    mut riders: Query<&mut Rider>,
) {
    if event.0.kind != EventKind::QuoteRejected {
        return;
    }

    let Some(EventSubject::Rider(rider_entity)) = event.0.subject else {
        return;
    };

    let Ok(mut rider) = riders.get_mut(rider_entity) else {
        return;
    };
    if rider.state != RiderState::Browsing {
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
        rider.state = RiderState::Cancelled;
        telemetry.riders_abandoned_quote_total = telemetry.riders_abandoned_quote_total.saturating_add(1);
        
        // Track breakdown by reason
        match rider.last_rejection_reason {
            Some(RiderAbandonmentReason::QuotePriceTooHigh) => {
                telemetry.riders_abandoned_price = telemetry.riders_abandoned_price.saturating_add(1);
            }
            Some(RiderAbandonmentReason::QuoteEtaTooLong) => {
                telemetry.riders_abandoned_eta = telemetry.riders_abandoned_eta.saturating_add(1);
            }
            Some(RiderAbandonmentReason::QuoteStochasticRejection) => {
                telemetry.riders_abandoned_stochastic = telemetry.riders_abandoned_stochastic.saturating_add(1);
            }
            _ => {
                // Fallback: if no reason recorded, count as stochastic (shouldn't happen in normal flow)
                telemetry.riders_abandoned_stochastic = telemetry.riders_abandoned_stochastic.saturating_add(1);
            }
        }
        
        commands.entity(rider_entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};
    use crate::ecs::Position;

    #[test]
    fn quote_rejected_under_limit_reschedules_show_quote() {
        use crate::test_helpers::test_neighbor_cell;

        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(SimTelemetry::default());
        world.insert_resource(RiderQuoteConfig {
            max_quote_rejections: 3,
            re_quote_delay_secs: 10,
            accept_probability: 0.8,
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
                    quote_rejections: 1,
                    accepted_fare: None,
                    last_rejection_reason: None,
                },
                Position(cell),
            ))
            .id();

        let now_ms = 5000;
        world.resource_mut::<SimulationClock>().schedule_at(
            now_ms,
            EventKind::QuoteRejected,
            Some(EventSubject::Rider(rider_entity)),
        );

        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("quote rejected event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems(quote_rejected_system);
        schedule.run(&mut world);

        let rider = world.get_entity(rider_entity).expect("rider still exists");
        let rider = rider.get::<Rider>().expect("Rider component");
        assert_eq!(rider.state, RiderState::Browsing);
        assert_eq!(rider.quote_rejections, 2);

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("show quote rescheduled");
        assert_eq!(next_event.kind, EventKind::ShowQuote);
        assert_eq!(next_event.subject, Some(EventSubject::Rider(rider_entity)));
        assert_eq!(next_event.timestamp, now_ms + 10 * 1000);
    }

    #[test]
    fn quote_rejected_at_limit_despawns_and_increments_telemetry() {
        use crate::test_helpers::test_neighbor_cell;

        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(SimTelemetry::default());
        world.insert_resource(RiderQuoteConfig {
            max_quote_rejections: 2,
            re_quote_delay_secs: 10,
            accept_probability: 0.8,
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
                    quote_rejections: 2, // Already at limit
                    accepted_fare: None,
                    last_rejection_reason: None,
                },
                Position(cell),
            ))
            .id();

        world.resource_mut::<SimulationClock>().schedule_at_secs(
            1,
            EventKind::QuoteRejected,
            Some(EventSubject::Rider(rider_entity)),
        );

        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("quote rejected event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems(quote_rejected_system);
        schedule.run(&mut world);

        let rider_exists = world.get_entity(rider_entity).is_some();
        assert!(!rider_exists, "rider should be despawned on give-up");

        let telemetry = world.resource::<SimTelemetry>();
        assert_eq!(telemetry.riders_abandoned_quote_total, 1);
    }
}
