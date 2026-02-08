//! Movement system: advances vehicles cell-by-cell along routes.
//!
//! On the first `MoveStep` for a trip, the route provider is queried and the
//! result is stored as a [`TripRoute`] component. Subsequent steps advance
//! along the cached cell path. Travel time per step is adjusted by the traffic
//! model (time-of-day profile, congestion zones, vehicle density).

use bevy_ecs::prelude::{Commands, Entity, ParamSet, Query, Res, ResMut, With};

use crate::clock::{CurrentEvent, EventKind, EventSubject, SimulationClock, ONE_SEC_MS};
use crate::ecs::{
    Driver, EnRoute, GeoPosition, OnTrip, Position, Rider, Trip, TripEnRoute, TripLiveData,
    TripOnTrip, TripRoute,
};
use crate::routing::RouteProviderResource;
use crate::spatial::{distance_km_between_cells, grid_path_cells_cached, SpatialIndex};
use crate::speed::{SpeedFactors, SpeedModel};
use crate::traffic::{
    compute_traffic_factor, CongestionZones, DynamicCongestionConfig, TrafficProfile,
};
use h3o::Resolution;

fn travel_time_ms(distance_km: f64, speed_kmh: f64) -> u64 {
    if distance_km <= 0.0 {
        ONE_SEC_MS
    } else {
        let hours = distance_km / speed_kmh.max(1.0);
        let ms = (hours * 60.0 * 60.0 * 1000.0).round() as u64;
        ms.max(ONE_SEC_MS)
    }
}
struct RouteStep {
    point: h3o::LatLng,
    distance_km: f64,
}

fn resolve_next_route_step(
    commands: &mut Commands,
    trip_entity: Entity,
    driver_pos_cell: h3o::CellIndex,
    target_cell: h3o::CellIndex,
    route_provider: &RouteProviderResource,
    trip_route: Option<&mut TripRoute>,
) -> Option<RouteStep> {
    if let Some(route) = trip_route {
        return route.advance().map(|(point, distance)| RouteStep {
            point,
            distance_km: distance,
        });
    }

    if let Some(route_result) = route_provider.0.route(driver_pos_cell, target_cell) {
        if let Some(mut new_route) = TripRoute::from_route_result(route_result) {
            if let Some((point, distance)) = new_route.advance() {
                commands.entity(trip_entity).insert(new_route);
                return Some(RouteStep {
                    point,
                    distance_km: distance,
                });
            }
        }
    }

    if let Some(path) = grid_path_cells_cached(driver_pos_cell, target_cell) {
        if let Some(mut new_route) = TripRoute::from_cells(path) {
            if let Some((point, distance)) = new_route.advance() {
                commands.entity(trip_entity).insert(new_route);
                return Some(RouteStep {
                    point,
                    distance_km: distance,
                });
            }
        }
    }

    None
}

fn lat_lng_to_cell(point: h3o::LatLng) -> h3o::CellIndex {
    point.to_cell(Resolution::Nine)
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
    mut trips: Query<(
        &mut Trip,
        &mut TripLiveData,
        Option<&mut TripRoute>,
        Option<&TripEnRoute>,
        Option<&TripOnTrip>,
    )>,
    mut queries: ParamSet<(
        Query<(
            &mut Driver,
            &mut Position,
            Option<&mut GeoPosition>,
            Option<&EnRoute>,
            Option<&OnTrip>,
        )>,
        Query<(&mut Position, Option<&mut GeoPosition>), With<Rider>>,
    )>,
) {
    if event.0.kind != EventKind::MoveStep {
        return;
    }

    let Some(EventSubject::Trip(trip_entity)) = event.0.subject else {
        return;
    };

    let (driver_entity, target_cell, is_en_route, rider_entity) = {
        let Ok((trip, _, _, en_route, on_trip)) = trips.get(trip_entity) else {
            return;
        };
        let is_en_route = en_route.is_some();
        if !is_en_route && on_trip.is_none() {
            return;
        }
        let target = if is_en_route {
            trip.pickup
        } else {
            trip.dropoff
        };
        (trip.driver, target, is_en_route, trip.rider)
    };

    let driver_pos_cell = {
        let driver_query = queries.p0();
        let Ok((_driver, driver_pos, _driver_geo, en_route, on_trip)) =
            driver_query.get(driver_entity)
        else {
            return;
        };
        if is_en_route && en_route.is_none() {
            return;
        }
        if !is_en_route && on_trip.is_none() {
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
            if let Ok((_, mut live_data, _, _, _)) = trips.get_mut(trip_entity) {
                live_data.pickup_eta_ms = 0;
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

    let route_step = {
        let mut trip_route = trips
            .get_mut(trip_entity)
            .ok()
            .and_then(|(_, _, route, _, _)| route);
        let trip_route_ref = trip_route.as_deref_mut();
        resolve_next_route_step(
            &mut commands,
            trip_entity,
            driver_pos_cell,
            target_cell,
            &route_provider,
            trip_route_ref,
        )
    };

    let RouteStep {
        point: next_geo,
        distance_km: step_distance_km,
    } = match route_step {
        Some(step) => step,
        None => {
            let kind = if is_en_route {
                EventKind::TripStarted
            } else {
                EventKind::TripCompleted
            };
            clock.schedule_in_secs(1, kind, Some(EventSubject::Trip(trip_entity)));
            return;
        }
    };

    let next_driver_cell = lat_lng_to_cell(next_geo);

    // Update driver position and precise geo location
    {
        let mut driver_query = queries.p0();
        let Ok((_, mut driver_pos, driver_geo, _, _)) = driver_query.get_mut(driver_entity) else {
            return;
        };
        driver_pos.0 = next_driver_cell;
        if let Some(mut geo) = driver_geo {
            geo.0 = next_geo;
        }
    }

    // If trip is OnTrip, update rider position to match driver (rider is in the vehicle)
    if !is_en_route {
        let mut rider_query = queries.p1();
        if let Ok((mut rider_pos, rider_geo)) = rider_query.get_mut(rider_entity) {
            rider_pos.0 = next_driver_cell;
            if let Some(mut geo) = rider_geo {
                geo.0 = next_geo;
            }
        }
    }

    let remaining = {
        if let Ok((_, mut live_data, route, _, _)) = trips.get_mut(trip_entity) {
            let remaining_distance = route
                .as_ref()
                .map(|route| route.remaining_distance_km())
                .unwrap_or_else(|| distance_km_between_cells(next_driver_cell, target_cell));
            if is_en_route {
                live_data.pickup_eta_ms = if remaining_distance <= 0.0 {
                    0
                } else {
                    travel_time_ms(remaining_distance, speed_kmh)
                };
            }
            remaining_distance
        } else {
            distance_km_between_cells(next_driver_cell, target_cell)
        }
    };

    if is_en_route {
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
    use crate::ecs::{
        EnRoute, GeoPosition, Rider, TripEnRoute, TripFinancials, TripLiveData, TripTiming, Waiting,
    };
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
                    matched_driver: None,
                    assigned_trip: None,
                    destination: Some(dropoff),
                    requested_at: None,
                    quote_rejections: 0,
                    accepted_fare: None,
                    last_rejection_reason: None,
                },
                Waiting,
                Position(neighbor),
                GeoPosition(neighbor.into()),
            ))
            .id();
        let driver_entity = world
            .spawn((
                Driver {
                    matched_rider: Some(rider_entity),
                    assigned_trip: None,
                },
                EnRoute,
                Position(origin),
                GeoPosition(origin.into()),
            ))
            .id();
        let trip_entity = world
            .spawn((
                Trip {
                    rider: rider_entity,
                    driver: driver_entity,
                    pickup: neighbor,
                    dropoff,
                },
                TripEnRoute,
                TripTiming {
                    requested_at: 0,
                    matched_at: 0,
                    pickup_at: None,
                    dropoff_at: None,
                    cancelled_at: None,
                },
                TripFinancials {
                    agreed_fare: None,
                    pickup_distance_km_at_accept: 0.0,
                },
                TripLiveData { pickup_eta_ms: 0 },
            ))
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
        assert_ne!(driver_position, origin);

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
            .expect("next event");
        assert!(
            matches!(
                next_event.kind,
                EventKind::MoveStep | EventKind::TripStarted
            ),
            "unexpected next event: {:?}",
            next_event.kind
        );
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
