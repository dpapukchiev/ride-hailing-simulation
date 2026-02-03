use bevy_ecs::prelude::Resource;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

#[derive(Debug, Clone, Copy)]
pub struct SpeedFactors {
    pub multiplier: f64,
}

impl Default for SpeedFactors {
    fn default() -> Self {
        Self { multiplier: 1.0 }
    }
}

#[derive(Resource)]
pub struct SpeedModel {
    rng: StdRng,
    min_kmh: f64,
    max_kmh: f64,
}

impl SpeedModel {
    pub fn new(seed: Option<u64>) -> Self {
        Self::with_range(seed, 20.0, 60.0)
    }

    pub fn with_range(seed: Option<u64>, min_kmh: f64, max_kmh: f64) -> Self {
        let rng = match seed {
            Some(seed) => StdRng::seed_from_u64(seed),
            None => StdRng::from_entropy(),
        };
        Self {
            rng,
            min_kmh,
            max_kmh,
        }
    }

    pub fn sample_kmh(&mut self, factors: SpeedFactors) -> f64 {
        let base = self.rng.gen_range(self.min_kmh..=self.max_kmh);
        (base * factors.multiplier).max(1.0)
    }
}
