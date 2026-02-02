use bevy_ecs::prelude::{Component, Entity};
use h3o::CellIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiderState {
    Requesting,
    Browsing,
    Waiting,
    InTransit,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct Rider {
    pub state: RiderState,
    pub matched_driver: Option<Entity>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct Position(pub CellIndex);
