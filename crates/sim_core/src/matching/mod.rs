pub mod algorithm;
pub mod types;
pub mod simple;
pub mod cost_based;

use bevy_ecs::prelude::Resource;

pub use algorithm::MatchingAlgorithm;
pub use types::{MatchCandidate, MatchResult};
pub use simple::SimpleMatching;
pub use cost_based::CostBasedMatching;

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
