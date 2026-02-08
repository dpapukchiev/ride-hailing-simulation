//! Entity Component System: components and state enums for riders, drivers, and trips.
//!
//! This module defines the core data structures used in the simulation:
//!
//! - **Components**: `Rider`, `Driver`, `Trip`, `Position`, `DriverEarnings`, `DriverFatigue`
//! - **State Markers**: rider/driver/trip marker components (e.g. `Browsing`, `Idle`, `TripEnRoute`)
//!
//! Components are attached to entities in the ECS world, and systems query/modify them
//! based on events. States represent the lifecycle stage of each entity.

use bevy_ecs::prelude::{Component, Entity};
use bevy_ecs::system::EntityCommands;
use h3o::{CellIndex, LatLng};

use crate::routing::RouteResult;
use crate::spatial::distance_km_between_lat_lng;
use crate::telemetry::RiderAbandonmentReason;

// Rider state markers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct Browsing;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct Waiting;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct InTransit;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct RiderCompleted;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct RiderCancelled;

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct Rider {
    pub matched_driver: Option<Entity>,
    /// Backlink to the active Trip entity. Set when a Trip is spawned
    /// (driver_decision_system), cleared on trip completion/cancellation.
    /// Enables O(1) trip lookup instead of scanning all Trip entities.
    pub assigned_trip: Option<Entity>,
    /// Requested dropoff cell. Must be set; riders without a destination are rejected by driver_decision_system.
    pub destination: Option<CellIndex>,
    /// Simulation time when the rider was spawned (set by request_inbound_system).
    pub requested_at: Option<u64>,
    /// Number of times this rider has rejected a quote (used for give-up after max_quote_rejections).
    pub quote_rejections: u32,
    /// Fare the rider accepted when they transitioned to Waiting; used for driver earnings and trip completion.
    pub accepted_fare: Option<f64>,
    /// Last reason this rider rejected a quote (used to track abandonment reason when they give up).
    pub last_rejection_reason: Option<RiderAbandonmentReason>,
}

/// Current quote shown to a rider (fare + ETA). Attached while rider is viewing a quote; used for UI/telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct RiderQuote {
    /// Quoted fare for the trip.
    pub fare: f64,
    /// Estimated time to pickup in milliseconds.
    pub eta_ms: u64,
}

// Driver state markers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct Idle;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct Evaluating;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct EnRoute;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct OnTrip;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct OffDuty;

/// Extension trait to transition a driver entity to a single state by clearing all driver state
/// markers and inserting the target. Use this instead of manually removing Idle/Evaluating/EnRoute/OnTrip/OffDuty.
pub trait DriverStateCommands {
    fn set_driver_state_idle(&mut self) -> &mut Self;
    fn set_driver_state_evaluating(&mut self) -> &mut Self;
    fn set_driver_state_en_route(&mut self) -> &mut Self;
    fn set_driver_state_on_trip(&mut self) -> &mut Self;
    fn set_driver_state_off_duty(&mut self) -> &mut Self;
}

impl<'a> DriverStateCommands for EntityCommands<'a> {
    fn set_driver_state_idle(&mut self) -> &mut Self {
        self.remove::<Idle>()
            .remove::<Evaluating>()
            .remove::<EnRoute>()
            .remove::<OnTrip>()
            .remove::<OffDuty>()
            .insert(Idle)
    }
    fn set_driver_state_evaluating(&mut self) -> &mut Self {
        self.remove::<Idle>()
            .remove::<Evaluating>()
            .remove::<EnRoute>()
            .remove::<OnTrip>()
            .remove::<OffDuty>()
            .insert(Evaluating)
    }
    fn set_driver_state_en_route(&mut self) -> &mut Self {
        self.remove::<Idle>()
            .remove::<Evaluating>()
            .remove::<EnRoute>()
            .remove::<OnTrip>()
            .remove::<OffDuty>()
            .insert(EnRoute)
    }
    fn set_driver_state_on_trip(&mut self) -> &mut Self {
        self.remove::<Idle>()
            .remove::<Evaluating>()
            .remove::<EnRoute>()
            .remove::<OnTrip>()
            .remove::<OffDuty>()
            .insert(OnTrip)
    }
    fn set_driver_state_off_duty(&mut self) -> &mut Self {
        self.remove::<Idle>()
            .remove::<Evaluating>()
            .remove::<EnRoute>()
            .remove::<OnTrip>()
            .remove::<OffDuty>()
            .insert(OffDuty)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct Driver {
    pub matched_rider: Option<Entity>,
    /// Backlink to the active Trip entity. Set when a Trip is spawned
    /// (driver_decision_system), cleared on trip completion/cancellation.
    /// Enables O(1) trip lookup instead of scanning all Trip entities.
    pub assigned_trip: Option<Entity>,
}

/// Tracks driver earnings and daily targets.
#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct DriverEarnings {
    /// Accumulated earnings for the current day.
    pub daily_earnings: f64,
    /// Earnings target at which driver goes OffDuty.
    pub daily_earnings_target: f64,
    /// Simulation time when driver started their current session (for fatigue calculation).
    pub session_start_time_ms: u64,
    /// Simulation time when driver went OffDuty (`None` while still active).
    pub session_end_time_ms: Option<u64>,
}

/// Tracks driver fatigue thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct DriverFatigue {
    /// Maximum time on duty before going OffDuty (in milliseconds).
    pub fatigue_threshold_ms: u64,
}

// Trip state markers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct TripEnRoute;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct TripOnTrip;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct TripCompleted;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct TripCancelled;

/// Core trip identity and spatial data (slimmed from the original 13-field Trip).
#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct Trip {
    pub rider: Entity,
    pub driver: Entity,
    pub pickup: CellIndex,
    pub dropoff: CellIndex,
}

/// Trip timing data: timestamps for the trip lifecycle funnel.
#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct TripTiming {
    /// Simulation time when the rider's request was received (Rider.requested_at).
    pub requested_at: u64,
    /// Simulation time when the driver accepted (Trip created).
    pub matched_at: u64,
    /// Simulation time when the driver reached pickup and trip started; set in trip_started_system.
    pub pickup_at: Option<u64>,
    /// Simulation time when the driver reached dropoff (trip completed); set in trip_completed_system.
    pub dropoff_at: Option<u64>,
    /// Simulation time when the trip was cancelled; set in rider_cancel_system.
    pub cancelled_at: Option<u64>,
}

/// Trip financial data: fare and distance metrics.
#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct TripFinancials {
    /// Agreed fare (quoted at accept time, may include surge). Used for driver earnings and platform revenue.
    pub agreed_fare: Option<f64>,
    /// Distance (km) from driver to pickup at match acceptance time.
    pub pickup_distance_km_at_accept: f64,
}

/// Trip live data: actively updated during en-route phase.
#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct TripLiveData {
    /// Estimated time to pickup from current driver position (ms), updated in movement_system.
    pub pickup_eta_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct Position(pub CellIndex);

/// Geographic location (latitude/longitude) for an entity.
#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct GeoPosition(pub LatLng);

impl From<CellIndex> for GeoPosition {
    fn from(cell: CellIndex) -> Self {
        Self(cell.into())
    }
}

/// Resolved route for a trip, stored on the Trip entity after the first MoveStep.
/// Contains route geometry so drivers advance along the actual path instead of raw
/// H3 hops.
#[derive(Debug, Clone, Component)]
pub struct TripRoute {
    /// Ordered lat/lng waypoints that describe the route (includes start + target).
    pub points: Vec<LatLng>,
    /// Distance (km) between consecutive waypoints.
    pub segment_distances_km: Vec<f64>,
    /// Index of the next segment to traverse (segment between points[i] and points[i+1]).
    pub next_segment_index: usize,
    /// Distance (km) already traveled along this route.
    pub distance_traveled_km: f64,
    /// Total route distance in km (from provider or segment sum).
    pub total_distance_km: f64,
}

impl TripRoute {
    fn from_points(points: Vec<LatLng>, total_distance_km: Option<f64>) -> Option<Self> {
        if points.len() < 2 {
            return None;
        }
        let mut segment_distances_km = Vec::with_capacity(points.len() - 1);
        let mut sum = 0.0;
        for window in points.windows(2) {
            let distance = distance_km_between_lat_lng(window[0], window[1]);
            segment_distances_km.push(distance);
            sum += distance;
        }
        if segment_distances_km.is_empty() {
            return None;
        }
        let mut total = total_distance_km.unwrap_or(sum);
        total = total.max(sum);
        Some(Self {
            points,
            segment_distances_km,
            next_segment_index: 0,
            distance_traveled_km: 0.0,
            total_distance_km: total,
        })
    }

    pub fn from_cells(cells: Vec<CellIndex>) -> Option<Self> {
        let points: Vec<LatLng> = cells.into_iter().map(|cell| cell.into()).collect();
        Self::from_points(points, None)
    }

    pub fn from_route_result(result: RouteResult) -> Option<Self> {
        let waypoints: Vec<LatLng> = result
            .waypoints
            .into_iter()
            .filter_map(|(lat, lng)| LatLng::new(lat, lng).ok())
            .collect();
        let points = if waypoints.len() >= 2 {
            waypoints
        } else {
            result.cells.into_iter().map(|cell| cell.into()).collect()
        };
        Self::from_points(points, Some(result.distance_km))
    }

    pub fn advance(&mut self) -> Option<(LatLng, f64)> {
        if self.next_segment_index >= self.segment_distances_km.len() {
            return None;
        }
        let distance = self.segment_distances_km[self.next_segment_index];
        let point = self.points[self.next_segment_index + 1];
        self.distance_traveled_km += distance;
        self.next_segment_index += 1;
        Some((point, distance))
    }

    pub fn remaining_distance_km(&self) -> f64 {
        (self.total_distance_km - self.distance_traveled_km).max(0.0)
    }
}
