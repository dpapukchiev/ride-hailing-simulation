//! Matching algorithms: pluggable strategies for driver-rider pairing.
//!
//! This module provides a trait-based system for implementing different matching
//! algorithms. Algorithms can optimize for different objectives:
//!
//! - **Distance**: Minimize pickup distance
//! - **ETA**: Minimize estimated time to pickup
//! - **Global optimization**: Batch matching for better overall efficiency
//!
//! ## Implementations
//!
//! - `SimpleMatching`: First available driver within radius
//! - `CostBasedMatching`: Scores drivers by distance and ETA
//!
//! ## Usage
//!
//! Algorithms are stored as a `MatchingAlgorithmResource` in the ECS world and can
//! be swapped dynamically during simulation execution.

pub mod algorithm;
pub mod types;
pub mod simple;
pub mod cost_based;

use bevy_ecs::prelude::Resource;

pub use algorithm::MatchingAlgorithm;
pub use types::{MatchCandidate, MatchResult};
pub use simple::SimpleMatching;
pub use cost_based::{CostBasedMatching, DEFAULT_ETA_WEIGHT};

/// Resource wrapper for the matching algorithm trait object.
#[derive(Resource)]
pub struct MatchingAlgorithmResource(pub Box<dyn MatchingAlgorithm>);

impl MatchingAlgorithmResource {
    pub fn new(algorithm: Box<dyn MatchingAlgorithm>) -> Self {
        Self(algorithm)
    }
}

impl std::ops::Deref for MatchingAlgorithmResource {
    type Target = dyn MatchingAlgorithm;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}
