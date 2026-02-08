//! Entity Component System: components and state enums for riders, drivers, and trips.
//!
//! This module defines the core data structures used in the simulation:
//!
//! - **Components**: `Rider`, `Driver`, `Trip`, `Position`, `DriverEarnings`, `DriverFatigue`
//! - **State Enums**: `RiderState`, `DriverState`, `TripState`
//!
//! Components are attached to entities in the ECS world, and systems query/modify them
//! based on events. States represent the lifecycle stage of each entity.

use bevy_ecs::prelude::{Component, Entity};
use h3o::CellIndex;

use crate::telemetry::RiderAbandonmentReason;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiderState {
    Browsing,
    Waiting,
    InTransit,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct Rider {
    pub state: RiderState,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverState {
    Idle,
    Evaluating,
    EnRoute,
    OnTrip,
    OffDuty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct Driver {
    pub state: DriverState,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TripState {
    EnRoute,
    OnTrip,
    Completed,
    Cancelled,
}

/// Core trip identity and spatial data (slimmed from the original 13-field Trip).
#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct Trip {
    pub state: TripState,
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

/// Resolved route for a trip, stored on the Trip entity after the first MoveStep.
/// Contains the full cell path so subsequent MoveSteps advance along it without
/// re-querying the route provider.
#[derive(Debug, Clone, Component)]
pub struct TripRoute {
    /// H3 cells from origin to destination.
    pub cells: Vec<CellIndex>,
    /// Current index into `cells` (the cell the driver is currently at or moving toward).
    pub current_index: usize,
    /// Total route distance in km (from the route provider, may differ from Haversine).
    pub total_distance_km: f64,
}
