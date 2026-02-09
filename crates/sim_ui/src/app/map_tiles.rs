use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};

mod bookkeeping;
mod fetch_decode;
mod projection_cache;
mod types;

use types::{CachedProjection, ProjectionBounds, TileResult};
pub use types::{MapSignature, TileGeometry, TileKey};

pub struct MapTileState {
    tiles: HashMap<TileKey, TileGeometry>,
    inflight: HashSet<TileKey>,
    errors: HashMap<TileKey, String>,
    sender: Sender<TileResult>,
    receiver: Receiver<TileResult>,
    last_signature: Option<MapSignature>,
    current_projection_bounds: Option<ProjectionBounds>,
    projection_cache: HashMap<TileKey, CachedProjection>,
}

impl MapTileState {
    pub fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        Self {
            tiles: HashMap::new(),
            inflight: HashSet::new(),
            errors: HashMap::new(),
            sender,
            receiver,
            last_signature: None,
            current_projection_bounds: None,
            projection_cache: HashMap::new(),
        }
    }

    pub fn update_signature(&mut self, signature: MapSignature) {
        if self.last_signature == Some(signature) {
            return;
        }
        self.tiles.clear();
        self.inflight.clear();
        self.errors.clear();
        self.current_projection_bounds =
            projection_cache::projection_bounds_from_signature(signature);
        self.projection_cache.clear();
        self.last_signature = Some(signature);
    }

    pub fn drain_results(&mut self) {
        while let Ok(result) = self.receiver.try_recv() {
            if let Some((key, geometry)) =
                bookkeeping::apply_tile_result(&mut self.inflight, &mut self.errors, result)
            {
                projection_cache::cache_projection_from_geometry(
                    &mut self.projection_cache,
                    self.current_projection_bounds,
                    key,
                    &geometry,
                );
                self.tiles.insert(key, geometry);
            }
        }
    }

    pub fn cached_projection_lines(&mut self, key: &TileKey) -> Option<&[Vec<(f32, f32)>]> {
        projection_cache::cached_projection_lines(&mut self.projection_cache, key)
    }

    pub fn cache_projection_from_geometry(&mut self, key: TileKey, geometry: &TileGeometry) {
        projection_cache::cache_projection_from_geometry(
            &mut self.projection_cache,
            self.current_projection_bounds,
            key,
            geometry,
        );
    }

    pub fn evict_stale_projections(&mut self) {
        bookkeeping::evict_stale_projections(&mut self.projection_cache);
    }

    pub fn request_missing_tiles<I>(&mut self, endpoint: &str, keys: I)
    where
        I: IntoIterator<Item = TileKey>,
    {
        let mut inflight_count = self.inflight.len();
        let endpoint = endpoint.trim_end_matches('/').to_string();
        for key in keys {
            if self.tiles.contains_key(&key) || self.inflight.contains(&key) {
                continue;
            }
            if self.errors.contains_key(&key) {
                continue;
            }
            let limit = bookkeeping::current_inflight_limit(self.tiles.len());
            if inflight_count >= limit {
                break;
            }
            inflight_count += 1;
            self.inflight.insert(key);
            let sender = self.sender.clone();
            let url = format!(
                "{}/tile/v1/driving/tile({},{},{}).mvt",
                endpoint, key.x, key.y, key.z
            );
            std::thread::spawn(move || {
                let result = fetch_decode::fetch_tile(&url, key);
                let _ = sender.send(result);
            });
        }
    }

    pub fn tile(&self, key: &TileKey) -> Option<&TileGeometry> {
        self.tiles.get(key)
    }
}
