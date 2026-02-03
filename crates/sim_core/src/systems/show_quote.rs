//! ShowQuote system: compute fare + ETA for a browsing rider and schedule quote decision.

use bevy_ecs::prelude::{Commands, Entity, Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Driver, DriverState, Position, Rider, RiderQuote, RiderState};
use crate::pricing::calculate_trip_fare;
use crate::spatial::distance_km_between_cells;

/// Default ETA in ms when no idle drivers are available (5 minutes).
const DEFAULT_ETA_MS: u64 = 300 * 1000;
/// Assumed speed for ETA from driver to rider (km/h).
const ETA_SPEED_KMH: f64 = 40.0;

pub fn show_quote_system(
    mut commands: Commands,
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    riders: Query<(Entity, &Rider, &Position)>,
    drivers: Query<(&Driver, &Position)>,
) {
    if event.0.kind != EventKind::ShowQuote {
        return;
    }

    let Some(EventSubject::Rider(rider_entity)) = event.0.subject else {
        return;
    };

    let Ok((_, rider, position)) = riders.get(rider_entity) else {
        return;
    };
    if rider.state != RiderState::Browsing {
        return;
    }
    let Some(dropoff) = rider.destination else {
        return;
    };

    let pickup = position.0;
    let fare = calculate_trip_fare(pickup, dropoff);

    let eta_ms = drivers
        .iter()
        .filter_map(|(driver, pos)| {
            if driver.state == DriverState::Idle {
                let distance_km = distance_km_between_cells(pos.0, pickup);
                let hours = distance_km / ETA_SPEED_KMH;
                Some((hours * 3_600_000.0) as u64)
            } else {
                None
            }
        })
        .min()
        .unwrap_or(DEFAULT_ETA_MS)
        .max(crate::clock::ONE_SEC_MS);

    commands
        .entity(rider_entity)
        .insert(RiderQuote { fare, eta_ms });

    clock.schedule_in_secs(
        1,
        EventKind::QuoteDecision,
        Some(EventSubject::Rider(rider_entity)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::{Schedule, World};

    #[test]
    fn show_quote_computes_quote_and_schedules_quote_decision() {
        use crate::test_helpers::test_neighbor_cell;

        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
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
                },
                Position(cell),
            ))
            .id();

        world.resource_mut::<SimulationClock>().schedule_at_secs(
            1,
            EventKind::ShowQuote,
            Some(EventSubject::Rider(rider_entity)),
        );

        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("show quote event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems(show_quote_system);
        schedule.run(&mut world);

        let rider_quote = world.get::<RiderQuote>(rider_entity).expect("RiderQuote");
        assert!(rider_quote.fare >= crate::pricing::BASE_FARE);
        assert!(rider_quote.eta_ms >= crate::clock::ONE_SEC_MS);

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("quote decision event");
        assert_eq!(next_event.kind, EventKind::QuoteDecision);
        assert_eq!(next_event.subject, Some(EventSubject::Rider(rider_entity)));
    }
}
