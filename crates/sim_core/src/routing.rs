//! Pluggable route providers: trait abstraction for routing backends.
//!
//! Three implementations, selectable via [`RouteProviderKind`]:
//!
//! - **`H3GridRouteProvider`**: Existing H3 grid-path + Haversine behavior. Zero dependencies.
//! - **`OsrmRouteProvider`** (feature `osrm`): Calls a local/remote OSRM HTTP endpoint.
//! - **`PrecomputedRouteProvider`** (feature `precomputed`): Loads a serialized route table from disk.
//!
//! The provider is stored as a `Box<dyn RouteProvider>` ECS resource, constructed from
//! `RouteProviderKind` during scenario building.

use bevy_ecs::prelude::Resource;
use h3o::CellIndex;
use serde::{Deserialize, Serialize};

use crate::spatial::{distance_km_between_cells, grid_path_cells_cached};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Serde helper: serialize a `Vec<CellIndex>` as `Vec<u64>`.
mod cell_vec_serde {
    use h3o::CellIndex;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(cells: &[CellIndex], ser: S) -> Result<S::Ok, S::Error> {
        let raw: Vec<u64> = cells.iter().map(|c| u64::from(*c)).collect();
        raw.serialize(ser)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Vec<CellIndex>, D::Error> {
        let raw: Vec<u64> = Vec::<u64>::deserialize(de)?;
        raw.into_iter()
            .map(|v| CellIndex::try_from(v).map_err(serde::de::Error::custom))
            .collect()
    }
}

/// Result of a route query between two H3 cells.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RouteResult {
    /// Lat/lng waypoints along the road (empty for H3 grid provider).
    pub waypoints: Vec<(f64, f64)>,
    /// Road-network distance in kilometres.
    pub distance_km: f64,
    /// Free-flow travel time in seconds (from OSRM or estimated).
    pub duration_secs: f64,
    /// H3 cells along the route used for step-by-step movement.
    #[serde(with = "cell_vec_serde")]
    pub cells: Vec<CellIndex>,
}

/// Which routing backend to use. Stored in [`ScenarioParams`] so it serializes
/// into the `ParameterSet` JSON that a future Lambda handler would receive.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub enum RouteProviderKind {
    /// Existing H3 grid-path behaviour, zero external dependencies.
    #[default]
    H3Grid,
    /// OSRM HTTP endpoint (e.g. `"http://localhost:5000"`).
    #[cfg(feature = "osrm")]
    Osrm { endpoint: String },
    /// Pre-computed route table loaded from a binary file at startup.
    #[cfg(feature = "precomputed")]
    Precomputed { path: String },
}

/// Trait for routing backends. Implementations must be `Send + Sync` so the
/// provider can be stored as a shared ECS resource.
pub trait RouteProvider: Send + Sync {
    /// Compute a route between two H3 cells. Returns `None` if no route exists.
    fn route(&self, from: CellIndex, to: CellIndex) -> Option<RouteResult>;
}

/// ECS resource wrapping a boxed route provider.
#[derive(Resource)]
pub struct RouteProviderResource(pub Box<dyn RouteProvider>);

// ---------------------------------------------------------------------------
// H3 Grid provider (always available)
// ---------------------------------------------------------------------------

/// Routes along the H3 hexagonal grid using `grid_path_cells` + Haversine distance.
/// This is the existing behaviour with zero external dependencies.
pub struct H3GridRouteProvider;

impl RouteProvider for H3GridRouteProvider {
    fn route(&self, from: CellIndex, to: CellIndex) -> Option<RouteResult> {
        let cells = grid_path_cells_cached(from, to)?;
        let distance_km = distance_km_between_cells(from, to);
        // Estimate free-flow duration at 40 km/h average city speed
        let duration_secs = if distance_km > 0.0 {
            (distance_km / 40.0) * 3600.0
        } else {
            0.0
        };
        Some(RouteResult {
            waypoints: Vec::new(),
            distance_km,
            duration_secs,
            cells,
        })
    }
}

// ---------------------------------------------------------------------------
// OSRM provider (behind `osrm` feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "osrm")]
pub mod osrm {
    use super::*;
    use reqwest::blocking::Client;
    use std::time::Duration;

    /// Routes via an OSRM HTTP endpoint.
    pub struct OsrmRouteProvider {
        client: Client,
        endpoint: String,
    }

    impl OsrmRouteProvider {
        pub fn new(endpoint: &str) -> Self {
            let client = Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("failed to build HTTP client");
            Self {
                client,
                endpoint: endpoint.trim_end_matches('/').to_string(),
            }
        }
    }

    /// Minimal OSRM JSON response structures.
    #[derive(Deserialize)]
    struct OsrmResponse {
        code: String,
        routes: Option<Vec<OsrmRoute>>,
    }

    #[derive(Deserialize)]
    struct OsrmRoute {
        distance: f64, // metres
        duration: f64, // seconds
        geometry: OsrmGeometry,
    }

    #[derive(Deserialize)]
    struct OsrmGeometry {
        coordinates: Vec<Vec<f64>>, // [lng, lat]
    }

    impl RouteProvider for OsrmRouteProvider {
        fn route(&self, from: CellIndex, to: CellIndex) -> Option<RouteResult> {
            let from_ll: h3o::LatLng = from.into();
            let to_ll: h3o::LatLng = to.into();

            let url = format!(
                "{}/route/v1/driving/{},{};{},{}?overview=full&geometries=geojson",
                self.endpoint,
                from_ll.lng(),
                from_ll.lat(),
                to_ll.lng(),
                to_ll.lat(),
            );

            let resp: OsrmResponse = match self.client.get(&url).send() {
                Ok(r) => match r.json() {
                    Ok(j) => j,
                    Err(_) => return None,
                },
                Err(_) => return None,
            };

            if resp.code != "Ok" {
                return None;
            }

            let route = resp.routes?.into_iter().next()?;

            let waypoints: Vec<(f64, f64)> = route
                .geometry
                .coordinates
                .iter()
                .map(|c| (c[1], c[0])) // OSRM returns [lng, lat], we store (lat, lng)
                .collect();

            // Snap waypoints to H3 resolution-9 cells
            let cells: Vec<CellIndex> = waypoints
                .iter()
                .filter_map(|&(lat, lng)| {
                    h3o::LatLng::new(lat, lng)
                        .ok()
                        .map(|ll| ll.to_cell(h3o::Resolution::Nine))
                })
                .collect();

            // Deduplicate consecutive identical cells
            let mut deduped_cells: Vec<CellIndex> = Vec::with_capacity(cells.len());
            for cell in cells {
                if deduped_cells.last() != Some(&cell) {
                    deduped_cells.push(cell);
                }
            }

            Some(RouteResult {
                waypoints,
                distance_km: route.distance / 1000.0,
                duration_secs: route.duration,
                cells: deduped_cells,
            })
        }
    }
}

#[cfg(feature = "osrm")]
pub mod osrm_spawn;

// ---------------------------------------------------------------------------
// Pre-computed provider (behind `precomputed` feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "precomputed")]
pub mod precomputed {
    use super::*;
    use std::collections::HashMap;
    use std::fs;

    /// A serializable key for the route table.
    /// CellIndex is a u64 internally, so we store the raw values.
    #[derive(Clone, Copy, Hash, Eq, PartialEq, Serialize, Deserialize)]
    pub struct CellPair(pub u64, pub u64);

    impl CellPair {
        pub fn new(from: CellIndex, to: CellIndex) -> Self {
            Self(from.into(), to.into())
        }
    }

    /// Pre-computed route table: a HashMap of cell-pair → RouteResult loaded from disk.
    pub struct PrecomputedRouteProvider {
        table: HashMap<CellPair, RouteResult>,
    }

    impl PrecomputedRouteProvider {
        /// Load from a bincode-serialized file.
        pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
            let data = fs::read(path)?;
            let table: HashMap<CellPair, RouteResult> = bincode::deserialize(&data)?;
            Ok(Self { table })
        }

        /// Create from an in-memory table (useful for tests).
        pub fn from_table(table: HashMap<CellPair, RouteResult>) -> Self {
            Self { table }
        }

        /// Serialize the table to a file.
        pub fn save_to_file(
            table: &HashMap<CellPair, RouteResult>,
            path: &str,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let data = bincode::serialize(table)?;
            fs::write(path, data)?;
            Ok(())
        }
    }

    impl RouteProvider for PrecomputedRouteProvider {
        fn route(&self, from: CellIndex, to: CellIndex) -> Option<RouteResult> {
            let key = CellPair::new(from, to);
            self.table.get(&key).cloned()
        }
    }
}

// ---------------------------------------------------------------------------
// Caching wrapper
// ---------------------------------------------------------------------------

use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Mutex;

/// LRU-cached wrapper around any [`RouteProvider`].
///
/// Cache key is `(from_cell_u64, to_cell_u64)` (directional).
/// On cache miss the inner provider is queried; on inner failure the optional
/// fallback (`H3GridRouteProvider`) is tried before returning `None`.
pub struct CachedRouteProvider {
    inner: Box<dyn RouteProvider>,
    cache: Mutex<LruCache<(u64, u64), RouteResult>>,
    fallback_to_h3: bool,
}

impl CachedRouteProvider {
    /// Create a caching wrapper with the given capacity.
    ///
    /// If `fallback_to_h3` is true, cache misses that also fail in the inner
    /// provider will be retried with [`H3GridRouteProvider`].
    pub fn new(inner: Box<dyn RouteProvider>, capacity: usize, fallback_to_h3: bool) -> Self {
        Self {
            inner,
            cache: Mutex::new(LruCache::new(
                NonZeroUsize::new(capacity.max(1)).expect("cache capacity must be > 0"),
            )),
            fallback_to_h3,
        }
    }
}

impl RouteProvider for CachedRouteProvider {
    fn route(&self, from: CellIndex, to: CellIndex) -> Option<RouteResult> {
        let key = (u64::from(from), u64::from(to));

        // Fast path: cache hit
        {
            let mut cache = self.cache.lock().ok()?;
            if let Some(cached) = cache.get(&key) {
                return Some(cached.clone());
            }
        }

        // Slow path: query inner provider
        let result = self.inner.route(from, to).or_else(|| {
            if self.fallback_to_h3 {
                H3GridRouteProvider.route(from, to)
            } else {
                None
            }
        });

        // Store in cache
        if let Some(ref route) = result {
            if let Ok(mut cache) = self.cache.lock() {
                cache.put(key, route.clone());
            }
        }

        result
    }
}

// ---------------------------------------------------------------------------
// Factory: build a provider from RouteProviderKind
// ---------------------------------------------------------------------------

/// Default route cache capacity (used by OSRM and precomputed providers).
#[cfg(any(feature = "osrm", feature = "precomputed"))]
const DEFAULT_ROUTE_CACHE_CAPACITY: usize = 20_000;

/// Construct a boxed [`RouteProvider`] from a [`RouteProviderKind`] descriptor.
///
/// - `H3Grid` is returned without caching (it's already fast and internally cached).
/// - `Osrm` and `Precomputed` providers are wrapped in a [`CachedRouteProvider`]
///   with H3 fallback on failure.
pub fn build_route_provider(kind: &RouteProviderKind) -> Box<dyn RouteProvider> {
    match kind {
        RouteProviderKind::H3Grid => Box::new(H3GridRouteProvider),

        #[cfg(feature = "osrm")]
        RouteProviderKind::Osrm { endpoint } => {
            let inner = Box::new(osrm::OsrmRouteProvider::new(endpoint));
            Box::new(CachedRouteProvider::new(
                inner,
                DEFAULT_ROUTE_CACHE_CAPACITY,
                true, // fallback to H3 on OSRM failure
            ))
        }

        #[cfg(feature = "precomputed")]
        RouteProviderKind::Precomputed { path } => {
            match precomputed::PrecomputedRouteProvider::from_file(path) {
                Ok(provider) => {
                    let inner = Box::new(provider);
                    Box::new(CachedRouteProvider::new(
                        inner,
                        DEFAULT_ROUTE_CACHE_CAPACITY,
                        true, // fallback to H3 on cache/table miss
                    ))
                }
                Err(e) => {
                    eprintln!(
                        "WARNING: Failed to load pre-computed route table from '{}': {}. Falling back to H3Grid.",
                        path, e
                    );
                    Box::new(H3GridRouteProvider)
                }
            }
        }
    }
}
