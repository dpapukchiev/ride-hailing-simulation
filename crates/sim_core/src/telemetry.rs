//! Telemetry / KPIs: records completed trips for analysis.

use std::collections::VecDeque;

use bevy_ecs::prelude::{Entity, Resource};
use h3o::CellIndex;

/// Rider lifecycle state (for telemetry/snapshot serialization).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiderState {
    Browsing,
    Waiting,
    InTransit,
    Completed,
    Cancelled,
}

/// Driver lifecycle state (for telemetry/snapshot serialization).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverState {
    Idle,
    Evaluating,
    EnRoute,
    OnTrip,
    OffDuty,
}

/// Trip lifecycle state (for telemetry/snapshot serialization).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TripState {
    EnRoute,
    OnTrip,
    Completed,
    Cancelled,
}

/// Reason why a rider abandoned their ride request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiderAbandonmentReason {
    /// Rider gave up after rejecting too many quotes due to price being too high.
    QuotePriceTooHigh,
    /// Rider gave up after rejecting too many quotes due to ETA being too long.
    QuoteEtaTooLong,
    /// Rider gave up after rejecting too many quotes due to stochastic rejection (accept_probability).
    QuoteStochasticRejection,
    /// Rider cancelled while waiting for pickup (timeout).
    PickupTimeout,
}

/// One completed trip, recorded when the driver reaches dropoff.
/// Timestamps are simulation ticks; use the helper methods for derived KPIs.
#[derive(Debug, Clone)]
pub struct CompletedTripRecord {
    pub trip_entity: Entity,
    pub rider_entity: Entity,
    pub driver_entity: Entity,
    pub completed_at: u64,
    pub requested_at: u64,
    pub matched_at: u64,
    pub pickup_at: u64,
    /// Fare paid (agreed at quote accept, may include surge).
    pub fare: f64,
    /// Additional cost due to surge pricing (fare - base_fare). Zero if no surge was applied.
    pub surge_impact: f64,
}

impl CompletedTripRecord {
    /// Time from request (Browsing) to driver acceptance.
    pub fn time_to_match(&self) -> u64 {
        self.matched_at.saturating_sub(self.requested_at)
    }

    /// Time from driver acceptance to pickup (trip started).
    pub fn time_to_pickup(&self) -> u64 {
        self.pickup_at.saturating_sub(self.matched_at)
    }

    /// Time from pickup to dropoff (passenger on board).
    pub fn trip_duration(&self) -> u64 {
        self.completed_at.saturating_sub(self.pickup_at)
    }
}

/// Collects simulation telemetry. Insert as a resource to record completed trips.
#[derive(Debug, Default, Resource)]
pub struct SimTelemetry {
    pub completed_trips: Vec<CompletedTripRecord>,
    pub riders_cancelled_total: u64,
    pub riders_completed_total: u64,
    /// Riders who gave up after rejecting too many quotes (distinct from pickup-timeout cancels).
    pub riders_abandoned_quote_total: u64,
    /// Breakdown of abandonment reasons for quote abandonments.
    pub riders_abandoned_price: u64,
    pub riders_abandoned_eta: u64,
    pub riders_abandoned_stochastic: u64,
    /// Breakdown of pickup cancellations.
    pub riders_cancelled_pickup_timeout: u64,
    /// Cumulative platform revenue from commission on completed trips.
    pub platform_revenue_total: f64,
    /// Total fares collected from riders (sum of agreed fares for completed trips).
    pub total_fares_collected: f64,
}

/// Snapshot of one rider for visualization/export.
#[derive(Debug, Clone)]
pub struct RiderSnapshot {
    pub entity: Entity,
    pub cell: CellIndex,
    pub state: RiderState,
    /// Driver entity if matched (None = waiting for match, Some = waiting for pickup)
    pub matched_driver: Option<Entity>,
}

/// Snapshot of one driver for visualization/export.
#[derive(Debug, Clone)]
pub struct DriverSnapshot {
    pub entity: Entity,
    pub cell: CellIndex,
    pub state: DriverState,
    /// Daily earnings (if available)
    pub daily_earnings: Option<f64>,
    /// Daily earnings target (if available)
    pub daily_earnings_target: Option<f64>,
    /// Session start time in ms (if available)
    pub session_start_time_ms: Option<u64>,
    /// Session end time in ms (set when driver goes OffDuty; `None` while active)
    pub session_end_time_ms: Option<u64>,
    /// Fatigue threshold in ms (if available)
    pub fatigue_threshold_ms: Option<u64>,
}

/// Snapshot of one trip for visualization/export.
#[derive(Debug, Clone)]
pub struct TripSnapshot {
    pub entity: Entity,
    pub rider: Entity,
    pub driver: Entity,
    pub state: TripState,
    pub pickup_cell: CellIndex,
    pub dropoff_cell: CellIndex,
    pub pickup_distance_km_at_accept: f64,
    pub requested_at: u64,
    pub matched_at: u64,
    pub pickup_at: Option<u64>,
    pub dropoff_at: Option<u64>,
    pub cancelled_at: Option<u64>,
}

/// Aggregated counts at a point in time.
#[derive(Debug, Clone, Default)]
pub struct SimCounts {
    pub riders_browsing: usize,
    pub riders_waiting: usize,
    pub riders_in_transit: usize,
    pub riders_completed: usize,
    pub riders_cancelled: usize,
    pub riders_cancelled_total: u64,
    pub riders_completed_total: u64,
    pub riders_abandoned_quote_total: u64,
    pub drivers_idle: usize,
    pub drivers_evaluating: usize,
    pub drivers_en_route: usize,
    pub drivers_on_trip: usize,
    pub drivers_off_duty: usize,
    pub trips_en_route: usize,
    pub trips_on_trip: usize,
    pub trips_completed: usize,
    pub trips_cancelled: usize,
}

/// Snapshot of simulation state at a specific timestamp (simulation ms).
#[derive(Debug, Clone)]
pub struct SimSnapshot {
    pub timestamp_ms: u64,
    pub counts: SimCounts,
    pub riders: Vec<RiderSnapshot>,
    pub drivers: Vec<DriverSnapshot>,
    pub trips: Vec<TripSnapshot>,
}

/// Snapshot capture configuration.
#[derive(Debug, Clone, Copy, Resource)]
pub struct SimSnapshotConfig {
    pub interval_ms: u64,
    pub max_snapshots: usize,
}

impl Default for SimSnapshotConfig {
    fn default() -> Self {
        Self {
            interval_ms: 1000,
            max_snapshots: 10_000,
        }
    }
}

/// Rolling snapshot buffer.
#[derive(Debug, Default, Resource)]
pub struct SimSnapshots {
    pub snapshots: VecDeque<SimSnapshot>,
    pub last_snapshot_at: Option<u64>,
}

impl SimCounts {
    pub fn add_rider(&mut self, state: RiderState) {
        match state {
            RiderState::Browsing => self.riders_browsing += 1,
            RiderState::Waiting => self.riders_waiting += 1,
            RiderState::InTransit => self.riders_in_transit += 1,
            RiderState::Completed => self.riders_completed += 1,
            RiderState::Cancelled => self.riders_cancelled += 1,
        }
    }

    pub fn add_driver(&mut self, state: DriverState) {
        match state {
            DriverState::Idle => self.drivers_idle += 1,
            DriverState::Evaluating => self.drivers_evaluating += 1,
            DriverState::EnRoute => self.drivers_en_route += 1,
            DriverState::OnTrip => self.drivers_on_trip += 1,
            DriverState::OffDuty => self.drivers_off_duty += 1,
        }
    }

    pub fn add_trip(&mut self, state: TripState) {
        match state {
            TripState::EnRoute => self.trips_en_route += 1,
            TripState::OnTrip => self.trips_on_trip += 1,
            TripState::Completed => self.trips_completed += 1,
            TripState::Cancelled => self.trips_cancelled += 1,
        }
    }
}
