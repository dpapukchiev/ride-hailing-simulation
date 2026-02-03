//! Probability distributions for spawner inter-arrival times.
//!
//! These distributions control the rate at which entities spawn, enabling
//! variable supply and demand patterns over time.

use rand::rngs::StdRng;
use rand::Rng;
use rand::SeedableRng;

/// Trait for sampling inter-arrival times (in milliseconds).
pub trait InterArrivalDistribution: Send + Sync + std::fmt::Debug {
    /// Sample the next inter-arrival time in milliseconds.
    /// `spawn_count` is the number of entities spawned so far (for time-varying distributions).
    fn sample_ms(&self, spawn_count: u64) -> f64;
}

/// Uniform distribution: constant inter-arrival time.
#[derive(Debug, Clone)]
pub struct UniformInterArrival {
    /// Inter-arrival time in milliseconds.
    pub interval_ms: f64,
}

impl UniformInterArrival {
    pub fn new(interval_ms: f64) -> Self {
        Self { interval_ms }
    }

    /// Create from rate (entities per second).
    pub fn from_rate(rate_per_sec: f64) -> Self {
        if rate_per_sec <= 0.0 {
            return Self { interval_ms: f64::INFINITY };
        }
        Self {
            interval_ms: 1000.0 / rate_per_sec,
        }
    }
}

impl InterArrivalDistribution for UniformInterArrival {
    fn sample_ms(&self, _spawn_count: u64) -> f64 {
        self.interval_ms
    }
}

/// Exponential distribution: Poisson process (constant rate, random inter-arrival times).
#[derive(Debug, Clone)]
pub struct ExponentialInterArrival {
    /// Rate parameter (lambda): expected number of events per second.
    pub rate_per_sec: f64,
    /// Seed for RNG (for reproducibility).
    pub seed: u64,
}

impl ExponentialInterArrival {
    pub fn new(rate_per_sec: f64, seed: u64) -> Self {
        Self {
            rate_per_sec: rate_per_sec.max(0.0),
            seed,
        }
    }
}

impl InterArrivalDistribution for ExponentialInterArrival {
    fn sample_ms(&self, spawn_count: u64) -> f64 {
        if self.rate_per_sec <= 0.0 {
            return f64::INFINITY;
        }
        // Use seeded RNG for reproducibility
        let mut rng = StdRng::seed_from_u64(self.seed.wrapping_add(spawn_count));
        // Sample from exponential: -ln(U) / lambda, where U is uniform [0,1)
        let u: f64 = rng.gen();
        let u = u.max(1e-10); // Avoid log(0)
        -u.ln() / self.rate_per_sec * 1000.0 // Convert to ms
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_inter_arrival_constant() {
        let dist = UniformInterArrival::new(1000.0);
        assert_eq!(dist.sample_ms(0), 1000.0);
        assert_eq!(dist.sample_ms(100), 1000.0);
    }

    #[test]
    fn uniform_from_rate() {
        let dist = UniformInterArrival::from_rate(2.0); // 2 per second
        assert_eq!(dist.interval_ms, 500.0);
    }

    #[test]
    fn exponential_inter_arrival() {
        let dist = ExponentialInterArrival::new(1.0, 42); // 1 per second
        let sample = dist.sample_ms(0);
        assert!(sample > 0.0);
        assert!(sample < 10000.0); // Reasonable upper bound for 1/sec rate
    }

    #[test]
    fn exponential_zero_rate() {
        let dist = ExponentialInterArrival::new(0.0, 42);
        assert_eq!(dist.sample_ms(0), f64::INFINITY);
    }

}
