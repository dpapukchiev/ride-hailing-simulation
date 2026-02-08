#![allow(dead_code)]

use bevy_ecs::prelude::World;
use sim_core::clock::SimulationClock;
use sim_core::pricing::PricingConfig;
use sim_core::routing::{build_route_provider, RouteProviderKind, RouteProviderResource};
use sim_core::scenario::{
    BatchMatchingConfig, DriverDecisionConfig, MatchRadius, RiderCancelConfig, RiderQuoteConfig,
};
use sim_core::spatial::SpatialIndex;
use sim_core::spawner::{SpawnWeighting, SpawnWeightingKind};
use sim_core::speed::SpeedModel;
use sim_core::telemetry::{SimSnapshotConfig, SimSnapshots, SimTelemetry};
use sim_core::traffic::{
    CongestionZones, DynamicCongestionConfig, TrafficProfile, TrafficProfileKind,
};

/// Builder configuration for reproducible test worlds.
#[derive(Clone, Debug)]
pub struct TestWorldConfig {
    pub seed: u64,
    pub match_radius: u32,
    pub traffic_profile: TrafficProfileKind,
    pub congestion_zones: Option<CongestionZones>,
    pub dynamic_congestion: bool,
    pub pricing_config: PricingConfig,
    pub spawn_weighting: SpawnWeightingKind,
    pub route_provider_kind: RouteProviderKind,
    pub speed_min_kmh: f64,
    pub speed_max_kmh: f64,
    pub use_spatial_index: bool,
}

impl Default for TestWorldConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            match_radius: 0,
            traffic_profile: TrafficProfileKind::default(),
            congestion_zones: None,
            dynamic_congestion: false,
            pricing_config: PricingConfig::default(),
            spawn_weighting: SpawnWeightingKind::default(),
            route_provider_kind: RouteProviderKind::default(),
            speed_min_kmh: 20.0,
            speed_max_kmh: 60.0,
            use_spatial_index: true,
        }
    }
}

/// Helper that populates the ECS world with all shared resources used in integration tests.
#[derive(Debug, Default)]
pub struct TestWorldBuilder {
    config: TestWorldConfig,
}

impl TestWorldBuilder {
    /// Create a new builder with default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the RNG seed used by all deterministically seeded helpers.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.config.seed = seed;
        self
    }

    /// Override the match radius used by matching systems.
    pub fn with_match_radius(mut self, radius: u32) -> Self {
        self.config.match_radius = radius;
        self
    }

    /// Choose a traffic profile for time-of-day multipliers.
    pub fn with_traffic_profile(mut self, profile: TrafficProfileKind) -> Self {
        self.config.traffic_profile = profile;
        self
    }

    /// Provide a pricing configuration (copy).
    pub fn with_pricing_config(mut self, pricing_config: PricingConfig) -> Self {
        self.config.pricing_config = pricing_config;
        self
    }

    /// Select a spawn weighting strategy for tests that exercise spawners.
    pub fn with_spawn_weighting(mut self, weighting: SpawnWeightingKind) -> Self {
        self.config.spawn_weighting = weighting;
        self
    }

    /// Choose the routing backend to insert.
    pub fn with_route_provider(mut self, kind: RouteProviderKind) -> Self {
        self.config.route_provider_kind = kind;
        self
    }

    /// Set a custom speed range for the movement speed model.
    pub fn with_speed_range(mut self, min_kmh: f64, max_kmh: f64) -> Self {
        self.config.speed_min_kmh = min_kmh;
        self.config.speed_max_kmh = max_kmh;
        self
    }

    /// Enable or disable spatial indexing (defaults to enabled).
    pub fn enable_spatial_index(mut self, enabled: bool) -> Self {
        self.config.use_spatial_index = enabled;
        self
    }

    /// Toggle dynamic congestion usage.
    pub fn with_dynamic_congestion(mut self, enabled: bool) -> Self {
        self.config.dynamic_congestion = enabled;
        self
    }

    /// Provide a prebuilt congestion zones map.
    pub fn with_congestion_zones(mut self, zones: CongestionZones) -> Self {
        self.config.congestion_zones = Some(zones);
        self
    }

    /// Build the ECS world with the configured resources.
    pub fn build(self) -> World {
        let TestWorldConfig {
            seed,
            match_radius,
            traffic_profile,
            congestion_zones,
            dynamic_congestion,
            pricing_config,
            spawn_weighting,
            route_provider_kind,
            speed_min_kmh,
            speed_max_kmh,
            use_spatial_index,
        } = self.config;

        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(SimTelemetry::default());
        world.insert_resource(SimSnapshotConfig::default());
        world.insert_resource(SimSnapshots::default());
        world.insert_resource(BatchMatchingConfig::default());
        world.insert_resource(MatchRadius(match_radius));
        world.insert_resource(RiderCancelConfig {
            seed: seed.wrapping_add(0xA5A5_A5A5),
            ..Default::default()
        });
        world.insert_resource(RiderQuoteConfig {
            seed: seed.wrapping_add(0x5A5A_5A5A),
            ..Default::default()
        });
        world.insert_resource(DriverDecisionConfig {
            seed: seed.wrapping_add(0xDEAD_BEEF),
            ..Default::default()
        });
        world.insert_resource(pricing_config);
        world.insert_resource(TrafficProfile::from_kind(&traffic_profile));
        let zones = congestion_zones.unwrap_or_else(CongestionZones::default);
        world.insert_resource(zones);
        world.insert_resource(DynamicCongestionConfig {
            enabled: dynamic_congestion,
        });
        world.insert_resource(SpawnWeighting::from_kind(&spawn_weighting));
        world.insert_resource(SpeedModel::with_range(
            Some(seed),
            speed_min_kmh,
            speed_max_kmh,
        ));
        if use_spatial_index {
            world.insert_resource(SpatialIndex::new());
        }
        let route_provider = build_route_provider(&route_provider_kind);
        world.insert_resource(RouteProviderResource(route_provider));
        world
    }
}
