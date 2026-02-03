use bevy_ecs::prelude::Entity;

/// Represents a potential driver-rider pairing with scoring information.
#[derive(Debug, Clone)]
pub struct MatchCandidate {
    pub rider_entity: Entity,
    pub driver_entity: Entity,
    pub pickup_distance_km: f64,
    pub pickup_eta_ms: u64,
    pub score: f64,
}

/// Represents a successful match result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchResult {
    pub rider_entity: Entity,
    pub driver_entity: Entity,
}
