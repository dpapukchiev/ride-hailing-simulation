//! ECS Systems: event-driven logic that reacts to simulation events.
//!
//! Systems are functions that query and mutate the ECS world based on the current
//! event. Each system handles one aspect of the simulation lifecycle:
//!
//! - **Spawners**: Create riders and drivers dynamically
//! - **Matching**: Pair riders with available drivers
//! - **Movement**: Move drivers toward pickup/dropoff locations
//! - **State Transitions**: Update entity states (browsing → waiting → matched, etc.)
//! - **Telemetry**: Capture snapshots for visualization/export
//!
//! Systems react to the `CurrentEvent` resource, which is inserted by the runner
//! before each schedule execution.

pub mod batch_matching;
pub mod driver_decision;
pub mod driver_offduty;
pub mod match_accepted;
pub mod match_rejected;
pub mod matching;
pub mod movement;
pub mod pickup_eta_updated;
pub mod quote_accepted;
pub mod quote_decision;
pub mod quote_rejected;
pub mod repositioning;
pub mod rider_cancel;
pub mod show_quote;
pub mod spatial_index;
pub mod spawner;
pub mod telemetry_snapshot;
pub mod trip_completed;
pub mod trip_started;
