//! Batch matching system: run a global matching pass when BatchMatchRun fires.
//!
//! Collects all riders in Waiting state and all Idle drivers, calls the matching
//! algorithm's find_batch_matches, applies matches, and schedules the next batch run.

use bevy_ecs::prelude::{Commands, Entity, Query, Res, ResMut};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock};
use crate::ecs::{Driver, DriverStateCommands, Idle, Position, Rider, Waiting};
use crate::matching::policy::{build_zone_counts, choose_best_driver};
use crate::matching::MatchingAlgorithmResource;
use crate::scenario::{BatchMatchingConfig, MatchRadius, RepositionPolicyConfig};

#[allow(clippy::too_many_arguments)]
pub fn batch_matching_system(
    mut commands: Commands,
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    batch_config: Option<Res<BatchMatchingConfig>>,
    match_radius: Option<Res<MatchRadius>>,
    matching_algorithm: Res<MatchingAlgorithmResource>,
    reposition_cfg: Option<Res<RepositionPolicyConfig>>,
    mut riders: Query<(Entity, &mut Rider, &Position, Option<&Waiting>)>,
    mut drivers: Query<(Entity, &mut Driver, &Position, Option<&Idle>)>,
) {
    if event.0.kind != EventKind::BatchMatchRun {
        return;
    }

    let Some(config) = batch_config.as_deref() else {
        return;
    };
    if !config.enabled {
        return;
    }

    let radius = match_radius.as_deref().map(|r| r.0).unwrap_or(0);

    // Collect only riders who are Waiting and not yet assigned (looking for a match).
    // Riders who are Waiting but already have matched_driver are waiting for that driver
    // to accept/drive to pickup and must not be re-matched.
    let waiting_riders: Vec<(Entity, h3o::CellIndex, Option<h3o::CellIndex>)> = riders
        .iter()
        .filter(|(_, rider, _, waiting)| waiting.is_some() && rider.matched_driver.is_none())
        .map(|(entity, rider, position, _)| (entity, position.0, rider.destination))
        .collect();

    // Collect all Idle drivers (exclude OffDuty and others)
    let available_drivers: Vec<(Entity, h3o::CellIndex)> = drivers
        .iter()
        .filter_map(|(entity, _driver, position, idle)| idle.map(|_| (entity, position.0)))
        .collect();

    let matches = if let Some(cfg) = reposition_cfg.as_deref() {
        let waiting_zone_demand = build_zone_counts(waiting_riders.iter().map(|(_, pos, _)| *pos));
        let mut idle_zone_supply = build_zone_counts(available_drivers.iter().map(|(_, pos)| *pos));
        let target_idle_by_zone = idle_zone_supply.clone();
        let mut remaining_drivers = available_drivers.clone();
        let mut greedy_matches = Vec::new();

        let mut riders_sorted = waiting_riders.clone();
        riders_sorted.sort_by_key(|(_, pos, _)| {
            std::cmp::Reverse(*waiting_zone_demand.get(pos).unwrap_or(&0))
        });

        for (rider_entity, rider_pos, _) in riders_sorted {
            let maybe_driver = choose_best_driver(
                rider_entity,
                rider_pos,
                &remaining_drivers,
                radius,
                &idle_zone_supply,
                &waiting_zone_demand,
                &target_idle_by_zone,
                cfg.minimum_zone_reserve,
                cfg.hotspot_weight,
            );
            let Some(driver_entity) = maybe_driver else {
                continue;
            };
            if let Some(idx) = remaining_drivers
                .iter()
                .position(|(entity, _)| *entity == driver_entity)
            {
                let (_, driver_pos) = remaining_drivers.remove(idx);
                *idle_zone_supply.entry(driver_pos).or_insert(0) = idle_zone_supply
                    .get(&driver_pos)
                    .copied()
                    .unwrap_or(0)
                    .saturating_sub(1);
                greedy_matches.push(crate::matching::MatchResult {
                    rider_entity,
                    driver_entity,
                });
            }
        }

        if greedy_matches.is_empty() && !waiting_riders.is_empty() {
            matching_algorithm.find_batch_matches(
                &waiting_riders,
                &available_drivers,
                radius,
                clock.now(),
            )
        } else {
            greedy_matches
        }
    } else {
        matching_algorithm.find_batch_matches(
            &waiting_riders,
            &available_drivers,
            radius,
            clock.now(),
        )
    };

    for m in matches {
        if let Ok((_, mut rider, _, _)) = riders.get_mut(m.rider_entity) {
            rider.matched_driver = Some(m.driver_entity);
        }
        if let Ok((_, mut driver, _, _)) = drivers.get_mut(m.driver_entity) {
            commands
                .entity(m.driver_entity)
                .set_driver_state_evaluating();
            driver.matched_rider = Some(m.rider_entity);
        }
        clock.schedule_in_secs(
            1,
            EventKind::MatchAccepted,
            Some(EventSubject::Driver(m.driver_entity)),
        );
    }

    // Schedule next batch run
    clock.schedule_in_secs(config.interval_secs, EventKind::BatchMatchRun, None);
}
