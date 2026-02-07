//! Traffic model: time-of-day speed profiles, spatial congestion zones,
//! and dynamic congestion from vehicle density.
//!
//! The traffic model modifies effective vehicle speed via multiplicative factors.
//! It is independent of the route provider -- it operates on speed/time, not
//! route geometry -- so it works with H3 grid paths, OSRM, or pre-computed routes.

use bevy_ecs::prelude::Resource;
use h3o::CellIndex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Traffic profile (time-of-day speed factors)
// ---------------------------------------------------------------------------

/// Pre-defined traffic profiles.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub enum TrafficProfileKind {
    /// No traffic effects; all hourly factors are 1.0.
    #[default]
    None,
    /// Realistic Berlin traffic pattern with rush-hour slowdowns.
    Berlin,
    /// Custom per-hour factors (index 0 = midnight, index 23 = 11 PM).
    Custom([f64; 24]),
}

/// Hourly speed multipliers. Factor 1.0 = free flow; 0.4 = heavy congestion.
#[derive(Clone, Debug, Resource)]
pub struct TrafficProfile {
    /// Speed multiplier for each hour of the day (0–23).
    pub hourly_factors: [f64; 24],
}

impl TrafficProfile {
    /// All factors 1.0 (no time-of-day effect).
    pub fn none() -> Self {
        Self {
            hourly_factors: [1.0; 24],
        }
    }

    /// Realistic Berlin traffic pattern.
    ///
    /// - 00–06: 1.0  (free flow, ~50 km/h)
    /// - 07–09: 0.45 (morning rush, ~22 km/h)
    /// - 09–16: 0.65 (midday, ~32 km/h)
    /// - 16–19: 0.40 (evening rush, ~20 km/h)
    /// - 19–23: 0.75 (evening, ~38 km/h)
    pub fn berlin() -> Self {
        let mut f = [1.0_f64; 24];
        // Morning rush
        f[7] = 0.45;
        f[8] = 0.45;
        // Midday
        for slot in &mut f[9..16] {
            *slot = 0.65;
        }
        // Evening rush
        f[16] = 0.40;
        f[17] = 0.40;
        f[18] = 0.40;
        // Evening
        f[19] = 0.75;
        f[20] = 0.75;
        f[21] = 0.75;
        f[22] = 0.75;
        f[23] = 0.75;
        Self { hourly_factors: f }
    }

    /// Build from a [`TrafficProfileKind`] descriptor.
    pub fn from_kind(kind: &TrafficProfileKind) -> Self {
        match kind {
            TrafficProfileKind::None => Self::none(),
            TrafficProfileKind::Berlin => Self::berlin(),
            TrafficProfileKind::Custom(factors) => Self {
                hourly_factors: *factors,
            },
        }
    }

    /// Look up the speed multiplier for a given simulation time.
    ///
    /// `sim_time_ms` is the current simulation clock value.
    /// `epoch_ms` is the real-world epoch (Unix ms) for simulation time 0.
    pub fn factor_at(&self, sim_time_ms: u64, epoch_ms: i64) -> f64 {
        let real_ms = epoch_ms + sim_time_ms as i64;
        // Convert to hour-of-day (UTC). For Berlin, epoch_ms should include timezone offset.
        let hour = ((real_ms / 3_600_000) % 24) as usize;
        self.hourly_factors[hour]
    }
}

// ---------------------------------------------------------------------------
// Spatial congestion zones
// ---------------------------------------------------------------------------

/// Configuration for spatial congestion zones.
#[derive(Clone, Debug, Default, Resource)]
pub struct CongestionZones {
    /// Per-cell speed multiplier overrides. Cells not in the map use 1.0.
    pub cell_factors: HashMap<CellIndex, f64>,
}

impl CongestionZones {
    /// Create Berlin default zones.
    ///
    /// - Mitte (city center): 0.70x (congested)
    /// - Surrounding ring: 0.85x
    ///
    /// These are approximate; a real implementation would load polygon data.
    pub fn berlin_defaults() -> Self {
        // For now, return empty — zone data requires specific H3 cells that
        // depend on the resolution. Populate from a data file in Phase 6.
        Self::default()
    }

    /// Look up the congestion factor for a cell. Returns 1.0 if not in a zone.
    pub fn factor_for_cell(&self, cell: CellIndex) -> f64 {
        self.cell_factors.get(&cell).copied().unwrap_or(1.0)
    }
}

// ---------------------------------------------------------------------------
// Dynamic congestion from vehicle density
// ---------------------------------------------------------------------------

/// Configuration for dynamic (density-based) congestion.
#[derive(Clone, Debug, Default, Resource)]
pub struct DynamicCongestionConfig {
    pub enabled: bool,
}

/// Compute a speed factor based on the number of drivers occupying a cell.
///
/// More drivers in the same ~240m hex → slower movement for everyone.
pub fn density_congestion_factor(drivers_in_cell: usize) -> f64 {
    match drivers_in_cell {
        0..=2 => 1.0,
        3..=5 => 0.85,
        6..=10 => 0.70,
        _ => 0.55,
    }
}

// ---------------------------------------------------------------------------
// Composite speed factor
// ---------------------------------------------------------------------------

/// Compute the combined traffic speed multiplier for a vehicle at a given
/// cell and simulation time. This is the product of:
///
/// 1. Time-of-day profile factor
/// 2. Spatial zone factor
/// 3. Dynamic density factor (if enabled)
pub fn compute_traffic_factor(
    profile: &TrafficProfile,
    zones: &CongestionZones,
    dynamic_config: &DynamicCongestionConfig,
    cell: CellIndex,
    sim_time_ms: u64,
    epoch_ms: i64,
    drivers_in_cell: usize,
) -> f64 {
    let time_factor = profile.factor_at(sim_time_ms, epoch_ms);
    let zone_factor = zones.factor_for_cell(cell);
    let density_factor = if dynamic_config.enabled {
        density_congestion_factor(drivers_in_cell)
    } else {
        1.0
    };
    time_factor * zone_factor * density_factor
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn berlin_profile_rush_hours_slower() {
        let p = TrafficProfile::berlin();
        // Night: free flow
        assert_eq!(p.hourly_factors[3], 1.0);
        // Morning rush
        assert!(p.hourly_factors[7] < 0.5);
        // Evening rush
        assert!(p.hourly_factors[17] < 0.5);
        // Midday
        assert!(p.hourly_factors[12] > 0.5);
        assert!(p.hourly_factors[12] < 1.0);
    }

    #[test]
    fn none_profile_all_ones() {
        let p = TrafficProfile::none();
        for f in &p.hourly_factors {
            assert_eq!(*f, 1.0);
        }
    }

    #[test]
    fn factor_at_uses_epoch() {
        let p = TrafficProfile::berlin();
        // epoch_ms = 0 means sim time 0 is Unix epoch (1970-01-01 00:00 UTC)
        // 7 hours in ms = 7 * 3600 * 1000 = 25_200_000
        let factor = p.factor_at(25_200_000, 0);
        assert_eq!(factor, 0.45); // hour 7 = morning rush
    }

    #[test]
    fn density_factor_scales() {
        assert_eq!(density_congestion_factor(0), 1.0);
        assert_eq!(density_congestion_factor(2), 1.0);
        assert_eq!(density_congestion_factor(4), 0.85);
        assert_eq!(density_congestion_factor(8), 0.70);
        assert_eq!(density_congestion_factor(15), 0.55);
    }

    #[test]
    fn composite_factor_multiplies() {
        let profile = TrafficProfile::berlin();
        let zones = CongestionZones::default();
        let config = DynamicCongestionConfig { enabled: true };
        let cell = CellIndex::try_from(0x8a1fb46622dffff_u64).expect("cell");

        // Hour 7 (rush), no zone override, 4 drivers in cell
        let factor = compute_traffic_factor(&profile, &zones, &config, cell, 25_200_000, 0, 4);
        // 0.45 (time) * 1.0 (zone) * 0.85 (density) ≈ 0.3825
        assert!((factor - 0.3825).abs() < 0.001);
    }

    #[test]
    fn from_kind_roundtrip() {
        let p = TrafficProfile::from_kind(&TrafficProfileKind::Berlin);
        assert_eq!(p.hourly_factors[7], 0.45);

        let p2 = TrafficProfile::from_kind(&TrafficProfileKind::None);
        assert_eq!(p2.hourly_factors[7], 1.0);

        let custom = [0.5; 24];
        let p3 = TrafficProfile::from_kind(&TrafficProfileKind::Custom(custom));
        assert_eq!(p3.hourly_factors[12], 0.5);
    }
}
