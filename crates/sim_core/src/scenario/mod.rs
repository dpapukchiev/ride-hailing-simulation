//! Scenario setup: configure spawners for riders and drivers.
//!
//! Uses spawners with inter-arrival time distributions to control spawn rates,
//! enabling variable supply and demand patterns.

mod build;
mod params;

pub use build::{
    build_scenario, create_cost_based_matching, create_hungarian_matching, create_simple_matching,
    random_destination,
};
pub use params::{
    BatchMatchingConfig, DriverDecisionConfig, MatchRadius, MatchingAlgorithmType,
    RepositionPolicyConfig, RiderCancelConfig, RiderQuoteConfig, ScenarioParams,
    SimulationEndTimeMs,
};
