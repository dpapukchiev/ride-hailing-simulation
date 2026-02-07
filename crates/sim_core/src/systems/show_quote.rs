//! ShowQuote system: compute fare + ETA for a browsing rider and schedule quote decision.

use bevy_ecs::prelude::{Commands, Entity, Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Driver, DriverState, Position, Rider, RiderQuote, RiderState};
use crate::pricing::{calculate_trip_fare_with_config, PricingConfig};
use crate::spatial::{distance_km_between_cells, grid_disk_cached, SpatialIndex};

/// Default ETA in ms when no idle drivers are available (5 minutes).
const DEFAULT_ETA_MS: u64 = 300 * 1000;
/// Assumed speed for ETA from driver to rider (km/h).
const ETA_SPEED_KMH: f64 = 40.0;

pub fn show_quote_system(
    mut commands: Commands,
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    pricing_config: Res<PricingConfig>,
    spatial_index: Option<Res<SpatialIndex>>,
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
    let base_fare = calculate_trip_fare_with_config(pickup, dropoff, *pricing_config);

    let surge_multiplier = if pricing_config.surge_enabled && pricing_config.surge_radius_k > 0 {
        let cluster_cells = grid_disk_cached(pickup, pricing_config.surge_radius_k);
        
        // Use spatial index if available, otherwise fall back to full scan
        let (demand, supply) = if let Some(index) = spatial_index.as_deref() {
            // Get entities in cluster cells from spatial index
            let rider_entities = index.get_riders_in_cells(&cluster_cells);
            let driver_entities = index.get_drivers_in_cells(&cluster_cells);
            
            // Query only those entities and filter by state
            let demand = rider_entities
                .iter()
                .filter_map(|&entity| riders.get(entity).ok())
                .filter(|(_, r, _)| r.state == RiderState::Browsing || r.state == RiderState::Waiting)
                .count();
            
            let supply = driver_entities
                .iter()
                .filter_map(|&entity| drivers.get(entity).ok())
                .filter(|(d, _)| d.state == DriverState::Idle)
                .count();
            
            (demand, supply)
        } else {
            // Fallback to full scan if spatial index not available
            let demand = riders
                .iter()
                .filter(|(_, r, pos)| {
                    (r.state == RiderState::Browsing || r.state == RiderState::Waiting)
                        && cluster_cells.contains(&pos.0)
                })
                .count();
            let supply = drivers
                .iter()
                .filter(|(d, pos)| d.state == DriverState::Idle && cluster_cells.contains(&pos.0))
                .count();
            (demand, supply)
        };
        
        if demand > supply && supply > 0 {
            let raw = 1.0 + (demand - supply) as f64 / supply as f64;
            raw.min(pricing_config.surge_max_multiplier)
        } else if demand > supply && supply == 0 {
            pricing_config.surge_max_multiplier
        } else {
            1.0
        }
    } else {
        1.0
    };

    let fare = base_fare * surge_multiplier;

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
        use crate::pricing::PricingConfig;
        use crate::test_helpers::test_neighbor_cell;

        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(PricingConfig::default());
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
