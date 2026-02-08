#![allow(dead_code)]

use bevy_ecs::prelude::{Entity, World};
use h3o::CellIndex;
use sim_core::ecs::{
    Browsing, Driver, DriverEarnings, DriverFatigue, EnRoute, Evaluating, GeoPosition, Idle,
    InTransit, OffDuty, OnTrip, Position, Rider, RiderCancelled, RiderCompleted, Waiting,
};
use sim_core::telemetry::RiderAbandonmentReason;
use sim_core::test_helpers::{test_cell, test_distant_cell, test_neighbor_cell};

/// Seeded helper cells so every test reuses the same geography.
pub fn seeded_cell() -> CellIndex {
    test_cell()
}

/// A nearby cell from the seeded geography.
pub fn seeded_neighbor_cell() -> CellIndex {
    test_neighbor_cell()
}

/// A more distant cell for dropoffs, reroutes, etc.
pub fn seeded_distant_cell() -> CellIndex {
    test_distant_cell()
}

/// Lifecycle states we can assign to rider entities during setup.
#[derive(Clone, Copy, Debug, Default)]
pub enum RiderLifecycleState {
    #[default]
    Browsing,
    Waiting,
    InTransit,
    Completed,
    Cancelled,
}

/// Lifecycle states we can assign to driver entities during setup.
#[derive(Clone, Copy, Debug, Default)]
pub enum DriverLifecycleState {
    #[default]
    Idle,
    Evaluating,
    EnRoute,
    OnTrip,
    OffDuty,
}

/// Builder for simple rider fixtures.
#[derive(Clone, Debug)]
pub struct RiderBuilder {
    position: CellIndex,
    destination: Option<CellIndex>,
    requested_at: u64,
    state: RiderLifecycleState,
    quote_rejections: u32,
    accepted_fare: Option<f64>,
    last_rejection_reason: Option<RiderAbandonmentReason>,
}

impl Default for RiderBuilder {
    fn default() -> Self {
        Self {
            position: seeded_cell(),
            destination: Some(seeded_neighbor_cell()),
            requested_at: 0,
            state: RiderLifecycleState::Browsing,
            quote_rejections: 0,
            accepted_fare: None,
            last_rejection_reason: None,
        }
    }
}

impl RiderBuilder {
    /// Create a fresh builder.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_position(mut self, cell: CellIndex) -> Self {
        self.position = cell;
        self
    }

    pub fn with_destination(mut self, cell: CellIndex) -> Self {
        self.destination = Some(cell);
        self
    }

    pub fn without_destination(mut self) -> Self {
        self.destination = None;
        self
    }

    pub fn with_requested_at(mut self, timestamp_ms: u64) -> Self {
        self.requested_at = timestamp_ms;
        self
    }

    pub fn with_state(mut self, state: RiderLifecycleState) -> Self {
        self.state = state;
        self
    }

    pub fn with_quote_rejections(mut self, count: u32) -> Self {
        self.quote_rejections = count;
        self
    }

    pub fn with_accepted_fare(mut self, fare: f64) -> Self {
        self.accepted_fare = Some(fare);
        self
    }

    pub fn with_last_rejection_reason(mut self, reason: RiderAbandonmentReason) -> Self {
        self.last_rejection_reason = Some(reason);
        self
    }

    /// Spawn the rider fixture into the provided world.
    pub fn spawn(self, world: &mut World) -> Entity {
        let mut entity = world.spawn((
            Rider {
                matched_driver: None,
                assigned_trip: None,
                destination: self.destination,
                requested_at: Some(self.requested_at),
                quote_rejections: self.quote_rejections,
                accepted_fare: self.accepted_fare,
                last_rejection_reason: self.last_rejection_reason,
            },
            Position(self.position),
            GeoPosition(self.position.into()),
        ));

        match self.state {
            RiderLifecycleState::Browsing => {
                entity.insert(Browsing);
            }
            RiderLifecycleState::Waiting => {
                entity.insert(Waiting);
            }
            RiderLifecycleState::InTransit => {
                entity.insert(InTransit);
            }
            RiderLifecycleState::Completed => {
                entity.insert(RiderCompleted);
            }
            RiderLifecycleState::Cancelled => {
                entity.insert(RiderCancelled);
            }
        }

        entity.id()
    }
}

/// Builder for simple driver fixtures.
#[derive(Clone, Debug)]
pub struct DriverBuilder {
    position: CellIndex,
    state: DriverLifecycleState,
    earnings_target: f64,
    fatigue_threshold_ms: u64,
}

impl Default for DriverBuilder {
    fn default() -> Self {
        Self {
            position: seeded_cell(),
            state: DriverLifecycleState::Idle,
            earnings_target: 100.0,
            fatigue_threshold_ms: 24 * 60 * 60 * 1000, // 24h
        }
    }
}

impl DriverBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_position(mut self, cell: CellIndex) -> Self {
        self.position = cell;
        self
    }

    pub fn with_state(mut self, state: DriverLifecycleState) -> Self {
        self.state = state;
        self
    }

    pub fn with_earnings_target(mut self, target: f64) -> Self {
        self.earnings_target = target;
        self
    }

    pub fn with_fatigue_threshold(mut self, threshold_ms: u64) -> Self {
        self.fatigue_threshold_ms = threshold_ms;
        self
    }

    /// Spawn the driver fixture into the provided world.
    pub fn spawn(self, world: &mut World) -> Entity {
        let mut entity = world.spawn((
            Driver {
                matched_rider: None,
                assigned_trip: None,
            },
            Position(self.position),
            GeoPosition(self.position.into()),
            DriverEarnings {
                daily_earnings: 0.0,
                daily_earnings_target: self.earnings_target,
                session_start_time_ms: 0,
                session_end_time_ms: None,
            },
            DriverFatigue {
                fatigue_threshold_ms: self.fatigue_threshold_ms,
            },
        ));

        match self.state {
            DriverLifecycleState::Idle => {
                entity.insert(Idle);
            }
            DriverLifecycleState::Evaluating => {
                entity.insert(Evaluating);
            }
            DriverLifecycleState::EnRoute => {
                entity.insert(EnRoute);
            }
            DriverLifecycleState::OnTrip => {
                entity.insert(OnTrip);
            }
            DriverLifecycleState::OffDuty => {
                entity.insert(OffDuty);
            }
        }

        entity.id()
    }
}
