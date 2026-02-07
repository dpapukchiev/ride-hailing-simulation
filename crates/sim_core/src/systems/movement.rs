//! Movement system: advances vehicles cell-by-cell along routes.
//!
//! On the first `MoveStep` for a trip, the route provider is queried and the
//! result is stored as a [`TripRoute`] component. Subsequent steps advance
//! along the cached cell path. Travel time per step is adjusted by the traffic
//! model (time-of-day profile, congestion zones, vehicle density).

use bevy_ecs::prelude::{Commands, Entity, ParamSet, Query, Res, ResMut, With};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock, ONE_SEC_MS};
use crate::ecs::{Driver, DriverState, Position, Rider, Trip, TripRoute, TripState};
use crate::routing::RouteProviderResource;
use crate::spatial::{distance_km_between_cells, grid_path_cells_cached, SpatialIndex};
use crate::speed::{SpeedFactors, SpeedModel};
use crate::traffic::{
    compute_traffic_factor, CongestionZones, DynamicCongestionConfig, TrafficProfile,
};

fn travel_time_ms(distance_km: f64, speed_kmh: f64) -> u64 {
    if distance_km <= 0.0 {
        ONE_SEC_MS
    } else {
        let hours = distance_km / speed_kmh.max(1.0);
        let ms = (hours * 60.0 * 60.0 * 1000.0).round() as u64;
        ms.max(ONE_SEC_MS)
    }
}

/// Resolve or advance the route for a trip, returning the next cell to move to.
///
/// - On first call (no `TripRoute`), queries the route provider and stores the result.
/// - On subsequent calls, advances `current_index` by one and returns the next cell.
fn resolve_next_cell(
    commands: &mut Commands,
    trip_entity: Entity,
    driver_pos_cell: h3o::CellIndex,
    target_cell: h3o::CellIndex,
    route_provider: &RouteProviderResource,
    trip_route: Option<&mut TripRoute>,
) -> Option<h3o::CellIndex> {
    if let Some(route) = trip_route {
        // Already have a route — advance to the next cell
        let next_idx = route.current_index + 1;
        if next_idx < route.cells.len() {
            route.current_index = next_idx;
            Some(route.cells[next_idx])
        } else {
            // Reached end of route
            None
        }
    } else {
        // First MoveStep: resolve route from provider
        let route_result = route_provider.0.route(driver_pos_cell, target_cell);

        if let Some(rr) = route_result {
            if rr.cells.len() > 1 {
                let next_cell = rr.cells[1];
                commands.entity(trip_entity).insert(TripRoute {
                    cells: rr.cells,
                    current_index: 1,
                    total_distance_km: rr.distance_km,
                });
                return Some(next_cell);
            }
        }

        // Fallback: use H3 grid path directly
        if let Some(path) = grid_path_cells_cached(driver_pos_cell, target_cell) {
            if let Some(next_cell) = path.get(1).copied() {
                let dist = distance_km_between_cells(driver_pos_cell, target_cell);
                commands.entity(trip_entity).insert(TripRoute {
                    cells: path,
                    current_index: 1,
                    total_distance_km: dist,
                });
                return Some(next_cell);
            }
        }

        None
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn movement_system(
    mut commands: Commands,
    mut clock: ResMut<SimulationClock>,
    event: Res<CurrentEvent>,
    mut speed: ResMut<SpeedModel>,
    route_provider: Res<RouteProviderResource>,
    traffic_profile: Res<TrafficProfile>,
    congestion_zones: Res<CongestionZones>,
    dynamic_congestion: Res<DynamicCongestionConfig>,
    spatial_index: Option<Res<SpatialIndex>>,
    mut trips: Query<(&mut Trip, Option<&mut TripRoute>)>,
    mut queries: ParamSet<(
        Query<(&mut Driver, &mut Position)>,
        Query<&mut Position, With<Rider>>,
    )>,
) {
    if event.0.kind != EventKind::MoveStep {
        return;
    }

    let Some(EventSubject::Trip(trip_entity)) = event.0.subject else {
        return;
    };

    let (driver_entity, target_cell, is_en_route, rider_entity) = {
        let Ok((trip, _)) = trips.get(trip_entity) else {
            return;
        };
        let target = match trip.state {
            TripState::EnRoute => trip.pickup,
            TripState::OnTrip => trip.dropoff,
            TripState::Completed | TripState::Cancelled => return,
        };
        (
            trip.driver,
            target,
            trip.state == TripState::EnRoute,
            trip.rider,
        )
    };

    let expected_state = if is_en_route {
        DriverState::EnRoute
    } else {
        DriverState::OnTrip
    };

    let driver_pos_cell = {
        let driver_query = queries.p0();
        let Ok((driver, driver_pos)) = driver_query.get(driver_entity) else {
            return;
        };
        if driver.state != expected_state {
            return;
        }
        driver_pos.0
    };

    // Compute traffic-adjusted speed
    let epoch_ms = clock.epoch_ms();
    let sim_time_ms = clock.now();
    let drivers_in_cell = spatial_index
        .as_ref()
        .map(|si| si.get_drivers_in_cells(&[driver_pos_cell]).len())
        .unwrap_or(0);

    let traffic_factor = compute_traffic_factor(
        &traffic_profile,
        &congestion_zones,
        &dynamic_congestion,
        driver_pos_cell,
        sim_time_ms,
        epoch_ms,
        drivers_in_cell,
    );

    let speed_kmh = speed.sample_kmh(SpeedFactors {
        multiplier: traffic_factor,
    });

    let remaining_km = distance_km_between_cells(driver_pos_cell, target_cell);
    if remaining_km <= 0.0 {
        if is_en_route {
            if let Ok((mut trip, _)) = trips.get_mut(trip_entity) {
                trip.pickup_eta_ms = 0;
            }
        }
        let kind = if is_en_route {
            EventKind::TripStarted
        } else {
            EventKind::TripCompleted
        };
        clock.schedule_in_secs(1, kind, Some(EventSubject::Trip(trip_entity)));
        return;
    }

    // Resolve next cell from route provider or cached route
    let next_driver_cell = {
        let mut trip_route = trips
            .get_mut(trip_entity)
            .ok()
            .and_then(|(_, route)| route);

        let trip_route_ref = trip_route.as_deref_mut();

        resolve_next_cell(
            &mut commands,
            trip_entity,
            driver_pos_cell,
            target_cell,
            &route_provider,
            trip_route_ref,
        )
        .unwrap_or(driver_pos_cell)
    };

    let step_distance_km = if next_driver_cell != driver_pos_cell {
        distance_km_between_cells(driver_pos_cell, next_driver_cell)
    } else {
        remaining_km
    };

    // Update driver position
    {
        let mut driver_query = queries.p0();
        let Ok((_, mut driver_pos)) = driver_query.get_mut(driver_entity) else {
            return;
        };
        driver_pos.0 = next_driver_cell;
    }

    // If trip is OnTrip, update rider position to match driver (rider is in the vehicle)
    if !is_en_route {
        let mut rider_query = queries.p1();
        if let Ok(mut rider_pos) = rider_query.get_mut(rider_entity) {
            rider_pos.0 = next_driver_cell;
        }
    }

    let remaining = distance_km_between_cells(next_driver_cell, target_cell);
    if is_en_route {
        if let Ok((mut trip, _)) = trips.get_mut(trip_entity) {
            trip.pickup_eta_ms = if remaining <= 0.0 {
                0
            } else {
                travel_time_ms(remaining, speed_kmh)
            };
        }
        clock.schedule_in(
            0,
            EventKind::PickupEtaUpdated,
            Some(EventSubject::Trip(trip_entity)),
        );
    }
    if remaining <= 0.0 {
        let kind = if is_en_route {
            EventKind::TripStarted
        } else {
            EventKind::TripCompleted
        };
        clock.schedule_in_secs(1, kind, Some(EventSubject::Trip(trip_entity)));
    } else {
        let step_ms = travel_time_ms(step_distance_km, speed_kmh);
        clock.schedule_in(
            step_ms,
            EventKind::MoveStep,
            Some(EventSubject::Trip(trip_entity)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::ONE_SEC_MS;
    use crate::ecs::{Rider, RiderState};
    use crate::routing::{H3GridRouteProvider, RouteProviderResource};
    use crate::speed::SpeedModel;
    use crate::traffic::{CongestionZones, DynamicCongestionConfig, TrafficProfile};
    use bevy_ecs::prelude::{Schedule, World};

    /// Helper to set up the world with all required resources for movement tests.
    fn setup_movement_world(world: &mut World) {
        world.insert_resource(SimulationClock::default());
        world.insert_resource(SpeedModel::with_range(Some(1), 40.0, 40.0));
        world.insert_resource(RouteProviderResource(Box::new(H3GridRouteProvider)));
        world.insert_resource(TrafficProfile::none());
        world.insert_resource(CongestionZones::default());
        world.insert_resource(DynamicCongestionConfig { enabled: false });
    }

    #[test]
    fn movement_steps_toward_rider_and_schedules_trip_start() {
        let mut world = World::new();
        setup_movement_world(&mut world);

        let origin = h3o::CellIndex::try_from(0x8a1fb46622dffff).expect("cell");
        let neighbor = origin
            .grid_disk::<Vec<_>>(1)
            .into_iter()
            .find(|cell| *cell != origin)
            .expect("neighbor");
        let dropoff = origin
            .grid_disk::<Vec<_>>(2)
            .into_iter()
            .find(|cell| *cell != origin && *cell != neighbor)
            .expect("dropoff cell");

        let rider_entity = world
            .spawn((
                Rider {
                    state: RiderState::Waiting,
                    matched_driver: None,
                    destination: Some(dropoff),
                    requested_at: None,
                    quote_rejections: 0,
                    accepted_fare: None,
                    last_rejection_reason: None,
                },
                Position(neighbor),
            ))
            .id();
        let driver_entity = world
            .spawn((
                Driver {
                    state: DriverState::EnRoute,
                    matched_rider: Some(rider_entity),
                },
                Position(origin),
            ))
            .id();
        let trip_entity = world
            .spawn(Trip {
                state: TripState::EnRoute,
                rider: rider_entity,
                driver: driver_entity,
                pickup: neighbor,
                dropoff,
                pickup_distance_km_at_accept: 0.0,
                requested_at: 0,
                matched_at: 0,
                pickup_at: None,
                pickup_eta_ms: 0,
                dropoff_at: None,
                cancelled_at: None,
                agreed_fare: None,
            })
            .id();

        world.resource_mut::<SimulationClock>().schedule_at_secs(
            1,
            EventKind::MoveStep,
            Some(EventSubject::Trip(trip_entity)),
        );
        let event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("move step event");
        world.insert_resource(CurrentEvent(event));

        let mut schedule = Schedule::default();
        schedule.add_systems(movement_system);
        schedule.run(&mut world);

        let driver_position = {
            let pos = world
                .query::<&Position>()
                .get(&world, driver_entity)
                .expect("pos");
            pos.0
        };
        assert_eq!(driver_position, neighbor);

        let eta_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("pickup eta updated event");
        assert_eq!(eta_event.kind, EventKind::PickupEtaUpdated);
        assert_eq!(eta_event.timestamp, 1000);
        assert_eq!(eta_event.subject, Some(EventSubject::Trip(trip_entity)));

        let next_event = world
            .resource_mut::<SimulationClock>()
            .pop_next()
            .expect("trip started event");
        assert_eq!(next_event.kind, EventKind::TripStarted);
        assert_eq!(next_event.timestamp, 2000);
        assert_eq!(next_event.subject, Some(EventSubject::Trip(trip_entity)));
    }

    #[test]
    fn eta_ms_scales_with_distance() {
        let speed = 40.0;
        assert_eq!(travel_time_ms(0.0, speed), ONE_SEC_MS);
        assert_eq!(travel_time_ms(1.0, speed), 90_000);
        assert_eq!(travel_time_ms(2.5, speed), 225_000);
    }
}
