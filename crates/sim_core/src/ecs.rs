use bevy_ecs::prelude::Component;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiderState {
    Requesting,
    WaitingForMatch,
    Matched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct Rider {
    pub state: RiderState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverState {
    Idle,
    Assigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct Driver {
    pub state: DriverState,
}
