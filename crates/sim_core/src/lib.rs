//! # Ride-Hailing Simulation Core
//!
//! A discrete-event simulation engine for modeling ride-hailing marketplaces.
//!
//! ## Overview
//!
//! This crate provides the core simulation engine, including:
//!
//! - **Event Scheduling**: Millisecond-precision discrete event system
//! - **ECS Framework**: Entity Component System for multi-agent state management
//! - **Spatial Indexing**: H3-based geographic operations
//! - **Matching Algorithms**: Pluggable driver-rider matching strategies
//! - **Telemetry**: Snapshot capture and data export
//!
//! ## Key Concepts
//!
//! - **Discrete Events**: All simulation progress happens through scheduled events
//! - **Targeted Events**: Events target specific entities (riders, drivers, trips)
//! - **Deterministic**: Seeded RNG ensures reproducible results
//! - **Scalable**: Efficient data structures support hundreds of concurrent agents
//!
//! ## Example
//!
//! ```rust,no_run
//! use bevy_ecs::prelude::World;
//! use sim_core::scenario::{build_scenario, ScenarioParams};
//! use sim_core::runner::{initialize_simulation, run_until_empty, simulation_schedule};
//!
//! let mut world = World::new();
//! build_scenario(&mut world, ScenarioParams::default().with_seed(42));
//! initialize_simulation(&mut world);
//!
//! let mut schedule = simulation_schedule();
//! let steps = run_until_empty(&mut world, &mut schedule, 1_000_000);
//! ```

pub mod spatial;
pub mod clock;
pub mod ecs;
pub mod speed;
pub mod runner;
pub mod scenario;
pub mod systems;
pub mod telemetry;
pub mod telemetry_export;
pub mod matching;
pub mod distributions;
pub mod spawner;
pub mod patterns;
pub mod pricing;

#[cfg(test)]
pub mod test_helpers;
