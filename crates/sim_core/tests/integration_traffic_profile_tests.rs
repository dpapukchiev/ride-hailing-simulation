mod support;

use h3o::CellIndex;
use sim_core::traffic::{
    compute_traffic_factor, density_congestion_factor, CongestionZones, DynamicCongestionConfig,
    TrafficProfile, TrafficProfileKind,
};

#[test]
fn berlin_profile_rush_hours_slower() {
    let p = TrafficProfile::berlin();
    assert_eq!(p.hourly_factors[3], 1.0);
    assert!(p.hourly_factors[7] < 0.5);
    assert!(p.hourly_factors[17] < 0.5);
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
    let factor = p.factor_at(25_200_000, 0);
    assert_eq!(factor, 0.45);
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

    let factor = compute_traffic_factor(&profile, &zones, &config, cell, 25_200_000, 0, 4);
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
