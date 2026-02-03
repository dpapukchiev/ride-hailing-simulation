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
    /// `current_time_ms` is the current simulation time in milliseconds (for time-of-day patterns).
    fn sample_ms(&self, spawn_count: u64, current_time_ms: u64) -> f64;
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
    fn sample_ms(&self, _spawn_count: u64, _current_time_ms: u64) -> f64 {
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
    fn sample_ms(&self, spawn_count: u64, _current_time_ms: u64) -> f64 {
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

/// Time-of-day and day-of-week aware distribution.
/// Varies the spawn rate based on hour of day (0-23) and day of week (0=Monday, 6=Sunday).
/// Uses a base rate multiplied by time-specific factors.
#[derive(Debug, Clone)]
pub struct TimeOfDayDistribution {
    /// Base rate (events per second) - this is the average rate.
    pub base_rate_per_sec: f64,
    /// Rate multipliers for each hour of day (0-23) and day of week (0=Monday, 6=Sunday).
    /// Indexed as [day_of_week][hour_of_day]. Default is 1.0 for all times.
    /// Example: multipliers[1][8] is the multiplier for Tuesday at 8 AM.
    pub multipliers: [[f64; 24]; 7],
    /// Epoch in milliseconds (real-world time corresponding to simulation time 0).
    /// Used to convert simulation time to real datetime for hour/day calculation.
    pub epoch_ms: i64,
    /// Seed for RNG (for reproducibility).
    pub seed: u64,
}

impl TimeOfDayDistribution {
    /// Create a new time-of-day distribution with default multipliers (all 1.0).
    pub fn new(base_rate_per_sec: f64, epoch_ms: i64, seed: u64) -> Self {
        Self {
            base_rate_per_sec: base_rate_per_sec.max(0.0),
            multipliers: [[1.0; 24]; 7],
            epoch_ms,
            seed,
        }
    }

    /// Set the multiplier for a specific day of week and hour.
    /// `day_of_week`: 0=Monday, 1=Tuesday, ..., 6=Sunday
    /// `hour`: 0-23
    pub fn set_multiplier(mut self, day_of_week: usize, hour: usize, multiplier: f64) -> Self {
        if day_of_week < 7 && hour < 24 {
            self.multipliers[day_of_week][hour] = multiplier.max(0.0);
        }
        self
    }

    /// Set multipliers for all hours of a specific day of week.
    pub fn set_day_multipliers(mut self, day_of_week: usize, multipliers: [f64; 24]) -> Self {
        if day_of_week < 7 {
            for (i, &mult) in multipliers.iter().enumerate() {
                if i < 24 {
                    self.multipliers[day_of_week][i] = mult.max(0.0);
                }
            }
        }
        self
    }

    /// Get the current rate multiplier based on simulation time.
    fn get_rate_multiplier(&self, sim_time_ms: u64) -> f64 {
        // Convert simulation time to real-world time
        let real_ms = self.epoch_ms.saturating_add(sim_time_ms as i64);
        
        // Convert to seconds since Unix epoch
        let total_secs = (real_ms / 1000) as i64;
        
        // Calculate day of week (0=Monday, 6=Sunday)
        // Unix epoch (1970-01-01) was a Thursday (day 3)
        let days_since_epoch = total_secs / 86400;
        let day_of_week = ((days_since_epoch + 3) % 7) as usize; // +3 because epoch was Thursday
        
        // Calculate hour of day (0-23) in UTC
        let secs_in_day = total_secs % 86400;
        let hour = ((secs_in_day / 3600) % 24) as usize;
        
        self.multipliers[day_of_week][hour]
    }
}

impl InterArrivalDistribution for TimeOfDayDistribution {
    fn sample_ms(&self, spawn_count: u64, current_time_ms: u64) -> f64 {
        let multiplier = self.get_rate_multiplier(current_time_ms);
        let adjusted_rate = self.base_rate_per_sec * multiplier;
        
        if adjusted_rate <= 0.0 {
            return f64::INFINITY;
        }
        
        // Use seeded RNG for reproducibility
        let mut rng = StdRng::seed_from_u64(self.seed.wrapping_add(spawn_count));
        // Sample from exponential: -ln(U) / lambda, where U is uniform [0,1)
        let u: f64 = rng.gen();
        let u = u.max(1e-10); // Avoid log(0)
        -u.ln() / adjusted_rate * 1000.0 // Convert to ms
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_inter_arrival_constant() {
        let dist = UniformInterArrival::new(1000.0);
        assert_eq!(dist.sample_ms(0, 0), 1000.0);
        assert_eq!(dist.sample_ms(100, 0), 1000.0);
    }

    #[test]
    fn uniform_from_rate() {
        let dist = UniformInterArrival::from_rate(2.0); // 2 per second
        assert_eq!(dist.interval_ms, 500.0);
    }

    #[test]
    fn exponential_inter_arrival() {
        let dist = ExponentialInterArrival::new(1.0, 42); // 1 per second
        let sample = dist.sample_ms(0, 0);
        assert!(sample > 0.0);
        assert!(sample < 10000.0); // Reasonable upper bound for 1/sec rate
    }

    #[test]
    fn exponential_zero_rate() {
        let dist = ExponentialInterArrival::new(0.0, 42);
        assert_eq!(dist.sample_ms(0, 0), f64::INFINITY);
    }

}
