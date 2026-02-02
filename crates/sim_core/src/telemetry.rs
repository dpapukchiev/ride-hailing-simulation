//! Telemetry / KPIs: records completed trips for analysis.

use bevy_ecs::prelude::{Entity, Resource};

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
}
