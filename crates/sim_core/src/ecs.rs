use bevy_ecs::prelude::{Component, Entity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiderState {
    Requesting,
    WaitingForMatch,
    Matched,
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
    Assigned,
    OnTrip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct Driver {
    pub state: DriverState,
    pub matched_rider: Option<Entity>,
}
