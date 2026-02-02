//! Telemetry / KPIs: records completed trips for analysis.

use bevy_ecs::prelude::{Entity, Resource};

/// One completed trip, recorded when the driver reaches dropoff.
#[derive(Debug, Clone)]
pub struct CompletedTripRecord {
    pub trip_entity: Entity,
    pub rider_entity: Entity,
    pub driver_entity: Entity,
    pub completed_at: u64,
}

/// Collects simulation telemetry. Insert as a resource to record completed trips.
#[derive(Debug, Default, Resource)]
pub struct SimTelemetry {
    pub completed_trips: Vec<CompletedTripRecord>,
}
