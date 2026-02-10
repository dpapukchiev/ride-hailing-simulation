use std::collections::HashMap;

use bevy_ecs::prelude::{Entity, ParamSet, Query, Res, ResMut, Resource};

use crate::clock::{CurrentEvent, EventKind, SimulationClock};
use crate::ecs::{Idle, Position, Rider, Waiting};
use crate::spatial::distance_km_between_cells;
use crate::{matching::policy::build_zone_counts, scenario::RepositionPolicyConfig};

#[derive(Default, Resource)]
pub struct RepositionState {
    pub cooldown_until_ms: HashMap<Entity, u64>,
}

fn compute_targets(
    zones: &[h3o::CellIndex],
    total_idle: usize,
    demand: &HashMap<h3o::CellIndex, usize>,
    cfg: &RepositionPolicyConfig,
) -> HashMap<h3o::CellIndex, usize> {
    if zones.is_empty() {
        return HashMap::new();
    }
    let base = (total_idle / zones.len()).max(cfg.minimum_zone_reserve);
    let total_demand: usize = demand.values().sum();

    zones
        .iter()
        .map(|zone| {
            let demand_share = if total_demand > 0 {
                *demand.get(zone).unwrap_or(&0) as f64 / total_demand as f64
            } else {
                0.0
            };
            let hotspot_extra =
                (demand_share * total_idle as f64 * cfg.hotspot_weight).round() as usize;
            (*zone, base + hotspot_extra)
        })
        .collect()
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn repositioning_system(
    event: Res<CurrentEvent>,
    mut clock: ResMut<SimulationClock>,
    cfg: Option<Res<RepositionPolicyConfig>>,
    mut state: ResMut<RepositionState>,
    mut queries: ParamSet<(
        Query<(&Position, Option<&Waiting>, &Rider)>,
        Query<(Entity, &mut Position, Option<&Idle>)>,
    )>,
) {
    if event.0.kind != EventKind::RepositionRun {
        return;
    }
    let Some(cfg) = cfg else {
        return;
    };

    let waiting_demand = build_zone_counts(
        queries
            .p0()
            .iter()
            .filter(|(_, waiting, rider)| waiting.is_some() && rider.matched_driver.is_none())
            .map(|(position, _, _)| position.0),
    );

    let idle_positions: Vec<(Entity, h3o::CellIndex)> = queries
        .p1()
        .iter_mut()
        .filter_map(|(entity, position, idle)| idle.map(|_| (entity, position.0)))
        .collect();
    let idle_supply = build_zone_counts(idle_positions.iter().map(|(_, cell)| *cell));

    let mut zones: Vec<h3o::CellIndex> = idle_supply
        .keys()
        .chain(waiting_demand.keys())
        .copied()
        .collect();
    zones.sort_unstable();
    zones.dedup();

    let target = compute_targets(&zones, idle_positions.len(), &waiting_demand, &cfg);

    let mut moved = 0usize;
    let now = clock.now();
    let mut assigned: HashMap<Entity, h3o::CellIndex> = HashMap::new();

    let mut deficits: Vec<(h3o::CellIndex, isize)> = zones
        .iter()
        .filter_map(|zone| {
            let supply = *idle_supply.get(zone).unwrap_or(&0) as isize;
            let desired = *target.get(zone).unwrap_or(&cfg.minimum_zone_reserve) as isize;
            let gap = desired - supply;
            (gap > 0).then_some((*zone, gap))
        })
        .collect();

    let mut supply_post = idle_supply.clone();

    for (deficit_zone, mut needed) in deficits.drain(..) {
        while needed > 0 && moved < cfg.max_drivers_moved_per_cycle {
            let candidate = idle_positions
                .iter()
                .filter(|(entity, from_zone)| {
                    if assigned.contains_key(entity) {
                        return false;
                    }
                    if now < *state.cooldown_until_ms.get(entity).unwrap_or(&0) {
                        return false;
                    }
                    let src_count = *supply_post.get(from_zone).unwrap_or(&0);
                    let src_target = *target.get(from_zone).unwrap_or(&cfg.minimum_zone_reserve);
                    src_count > src_target.max(cfg.minimum_zone_reserve)
                })
                .filter_map(|(entity, from_zone)| {
                    let dist = distance_km_between_cells(*from_zone, deficit_zone);
                    (dist <= cfg.max_reposition_distance_km).then_some((*entity, *from_zone, dist))
                })
                .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

            let Some((entity, from_zone, _)) = candidate else {
                break;
            };

            assigned.insert(entity, deficit_zone);
            *supply_post.entry(from_zone).or_insert(0) = supply_post
                .get(&from_zone)
                .copied()
                .unwrap_or(0)
                .saturating_sub(1);
            *supply_post.entry(deficit_zone).or_insert(0) += 1;
            state.cooldown_until_ms.insert(
                entity,
                now.saturating_add(cfg.cooldown_secs.saturating_mul(1_000)),
            );

            moved += 1;
            needed -= 1;
        }
    }

    if !assigned.is_empty() {
        for (entity, mut position, idle) in &mut queries.p1() {
            if idle.is_none() {
                continue;
            }
            if let Some(target_zone) = assigned.get(&entity) {
                position.0 = *target_zone;
            }
        }
    }

    clock.schedule_in_secs(cfg.control_interval_secs, EventKind::RepositionRun, None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_bias_toward_hotspot() {
        let zone_a = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("zone a");
        let zone_b = zone_a
            .grid_disk::<Vec<_>>(1)
            .into_iter()
            .find(|cell| *cell != zone_a)
            .expect("zone b");
        let zones = vec![zone_a, zone_b];

        let mut demand = HashMap::new();
        demand.insert(zone_a, 10);
        demand.insert(zone_b, 1);

        let cfg = RepositionPolicyConfig::default();
        let target = compute_targets(&zones, 20, &demand, &cfg);
        assert!(target[&zone_a] > target[&zone_b]);
    }
}
