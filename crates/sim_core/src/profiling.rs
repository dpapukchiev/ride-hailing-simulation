//! Performance profiling infrastructure: system timing, event rate tracking, and metrics collection.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use bevy_ecs::prelude::Resource;

use crate::clock::EventKind;

/// Per-system timing metrics.
#[derive(Debug, Clone, Default)]
pub struct SystemTiming {
    /// Total time spent in this system (cumulative).
    pub total_duration: Duration,
    /// Number of times this system was executed.
    pub call_count: u64,
    /// Minimum execution time.
    pub min_duration: Duration,
    /// Maximum execution time.
    pub max_duration: Duration,
}

impl SystemTiming {
    /// Record a system execution time.
    pub fn record(&mut self, duration: Duration) {
        self.total_duration += duration;
        self.call_count += 1;
        if duration < self.min_duration || self.min_duration == Duration::ZERO {
            self.min_duration = duration;
        }
        if duration > self.max_duration {
            self.max_duration = duration;
        }
    }

    /// Average execution time.
    pub fn avg_duration(&self) -> Duration {
        if self.call_count == 0 {
            Duration::ZERO
        } else {
            // Convert to nanoseconds for precise division, then back to Duration
            let total_nanos = self.total_duration.as_nanos();
            let avg_nanos = total_nanos / self.call_count as u128;
            Duration::from_nanos(avg_nanos as u64)
        }
    }
}

/// Aggregated system timing metrics.
#[derive(Debug, Default, Resource)]
pub struct SystemTimings {
    /// Timing data per system, keyed by system name.
    timings: HashMap<String, SystemTiming>,
}

impl SystemTimings {
    /// Record timing for a system.
    pub fn record(&mut self, system_name: &str, duration: Duration) {
        self.timings
            .entry(system_name.to_string())
            .or_insert_with(SystemTiming::default)
            .record(duration);
    }

    /// Get timing for a system.
    pub fn get(&self, system_name: &str) -> Option<&SystemTiming> {
        self.timings.get(system_name)
    }

    /// Get all timings.
    pub fn all(&self) -> &HashMap<String, SystemTiming> {
        &self.timings
    }

    /// Print summary statistics.
    pub fn print_summary(&self) {
        println!("\n=== System Timing Summary ===");
        let mut entries: Vec<_> = self.timings.iter().collect();
        entries.sort_by(|a, b| b.1.total_duration.cmp(&a.1.total_duration));

        for (name, timing) in entries {
            println!(
                "{:40} | calls: {:6} | total: {:8.2}ms | avg: {:6.2}μs | min: {:6.2}μs | max: {:6.2}μs",
                name,
                timing.call_count,
                timing.total_duration.as_secs_f64() * 1000.0,
                timing.avg_duration().as_secs_f64() * 1_000_000.0,
                timing.min_duration.as_secs_f64() * 1_000_000.0,
                timing.max_duration.as_secs_f64() * 1_000_000.0,
            );
        }
    }
}

/// Event processing rate metrics.
#[derive(Debug, Default, Resource)]
pub struct EventMetrics {
    /// Total events processed.
    pub events_processed: u64,
    /// Start time for rate calculation.
    pub start_time: Option<Instant>,
    /// Events per event kind.
    pub events_by_kind: HashMap<EventKind, u64>,
}

impl EventMetrics {
    /// Record an event being processed.
    pub fn record_event(&mut self, kind: EventKind) {
        if self.start_time.is_none() {
            self.start_time = Some(Instant::now());
        }
        self.events_processed += 1;
        *self.events_by_kind.entry(kind).or_insert(0) += 1;
    }

    /// Get current event processing rate (events per second).
    pub fn events_per_second(&self) -> f64 {
        if let Some(start) = self.start_time {
            let elapsed = start.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                self.events_processed as f64 / elapsed
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    /// Print summary statistics.
    pub fn print_summary(&self) {
        println!("\n=== Event Processing Summary ===");
        println!("Total events processed: {}", self.events_processed);
        if let Some(start) = self.start_time {
            let elapsed = start.elapsed();
            println!("Total time: {:.2}s", elapsed.as_secs_f64());
            println!("Events per second: {:.0}", self.events_per_second());
        }

        println!("\nEvents by kind:");
        let mut entries: Vec<_> = self.events_by_kind.iter().collect();
        entries.sort_by(|a, b| b.1.cmp(a.1));
        for (kind, count) in entries {
            println!("  {:30} : {}", format!("{:?}", kind), count);
        }
    }
}

/// Helper macro to time a system execution.
#[macro_export]
macro_rules! time_system {
    ($system_name:expr, $timings:expr, $body:block) => {{
        let start = std::time::Instant::now();
        let result = $body;
        let duration = start.elapsed();
        if let Some(timings) = $timings {
            timings.record($system_name, duration);
        }
        result
    }};
}
