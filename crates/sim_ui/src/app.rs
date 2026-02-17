//! Application state and core simulation wiring for the UI.

mod defaults;
mod map_tiles;
mod presets;
mod simulation;

pub use map_tiles::{MapSignature, TileKey};
pub use simulation::{MatchingAlgorithmType, RoutingMode, SimUiApp, SpawnMode, TrafficProfileMode};
