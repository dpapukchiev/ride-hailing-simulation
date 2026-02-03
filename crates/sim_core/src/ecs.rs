use bevy_ecs::prelude::{Component, Entity};
use h3o::CellIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiderState {
    Browsing,
    Waiting,
    InTransit,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct Rider {
    pub state: RiderState,
    pub matched_driver: Option<Entity>,
    /// Requested dropoff cell. Must be set; riders without a destination are rejected by driver_decision_system.
    pub destination: Option<CellIndex>,
    /// Simulation time when the rider was spawned (set by request_inbound_system).
    pub requested_at: Option<u64>,
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

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct Trip {
    pub state: TripState,
    pub rider: Entity,
    pub driver: Entity,
    pub pickup: CellIndex,
    pub dropoff: CellIndex,
    /// Distance (km) from driver to pickup at match acceptance time.
    pub pickup_distance_km_at_accept: f64,
    /// Simulation time when the rider's request was received (Rider.requested_at).
    pub requested_at: u64,
    /// Simulation time when the driver accepted (Trip created).
    pub matched_at: u64,
    /// Simulation time when the driver reached pickup and trip started; set in trip_started_system.
    pub pickup_at: Option<u64>,
    /// Estimated time to pickup from current driver position (ms), updated in movement_system.
    pub pickup_eta_ms: u64,
    /// Simulation time when the driver reached dropoff (trip completed); set in trip_completed_system.
    pub dropoff_at: Option<u64>,
    /// Simulation time when the trip was cancelled; set in rider_cancel_system.
    pub cancelled_at: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct Position(pub CellIndex);
