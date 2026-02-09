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

    #[test]
    fn eta_ms_scales_with_distance() {
        let speed = 40.0;
        assert_eq!(travel_time_ms(0.0, speed), ONE_SEC_MS);
        assert_eq!(travel_time_ms(1.0, speed), 90_000);
        assert_eq!(travel_time_ms(2.5, speed), 225_000);
    }
}
