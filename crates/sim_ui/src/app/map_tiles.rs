use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileKey {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MapSignature {
    pub z: u8,
    pub lat_min: i64,
    pub lat_max: i64,
    pub lng_min: i64,
    pub lng_max: i64,
}

#[derive(Clone, Copy)]
struct ProjectionBounds {
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
}

impl ProjectionBounds {
    fn new(lat_min: f64, lat_max: f64, lng_min: f64, lng_max: f64) -> Option<Self> {
        if lat_max > lat_min && lng_max > lng_min {
            Some(Self {
                lat_min,
                lat_max,
                lng_min,
                lng_max,
            })
        } else {
            None
        }
    }

    fn lat_span(&self) -> f64 {
        self.lat_max - self.lat_min
    }

    fn lng_span(&self) -> f64 {
        self.lng_max - self.lng_min
    }
}

struct CachedProjection {
    normalized_lines: Vec<Vec<(f32, f32)>>,
    last_used: Instant,
}

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

#[derive(Debug, Clone)]
pub struct TileGeometry {
    pub lines: Vec<Vec<(f64, f64)>>,
}

struct TileResult {
    key: TileKey,
    geometry: Option<TileGeometry>,
    error: Option<String>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
struct VectorTile {
    #[prost(message, repeated, tag = "3")]
    pub layers: Vec<VectorTileLayer>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
struct VectorTileLayer {
    #[prost(uint32, tag = "15")]
    pub version: u32,
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(message, repeated, tag = "2")]
    pub features: Vec<VectorTileFeature>,
    #[prost(uint32, tag = "5")]
    pub extent: u32,
}

#[derive(Clone, PartialEq, ::prost::Message)]
struct VectorTileFeature {
    #[prost(uint64, tag = "1")]
    pub id: u64,
    #[prost(uint32, repeated, packed = "true", tag = "2")]
    pub tags: Vec<u32>,
    #[prost(enumeration = "GeomType", tag = "3")]
    pub r#type: i32,
    #[prost(uint32, repeated, packed = "true", tag = "4")]
    pub geometry: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ::prost::Enumeration)]
#[repr(i32)]
enum GeomType {
    Unknown = 0,
    Point = 1,
    Linestring = 2,
    Polygon = 3,
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
        self.current_projection_bounds = ProjectionBounds::new(
            signature.lat_min as f64 / 1_000_000.0,
            signature.lat_max as f64 / 1_000_000.0,
            signature.lng_min as f64 / 1_000_000.0,
            signature.lng_max as f64 / 1_000_000.0,
        );
        self.projection_cache.clear();
        self.last_signature = Some(signature);
    }

    pub fn drain_results(&mut self) {
        while let Ok(result) = self.receiver.try_recv() {
            self.inflight.remove(&result.key);
            if let Some(error) = result.error {
                self.errors.insert(result.key, error);
                continue;
            }
            if let Some(geometry) = result.geometry {
                self.cache_projection_from_geometry(result.key, &geometry);
                self.tiles.insert(result.key, geometry);
            }
        }
    }

    fn current_inflight_limit(&self) -> usize {
        const WARMUP_TILES: usize = 6;
        const WARMUP_LIMIT: usize = 4;
        const MAX_LIMIT: usize = 12;
        if self.tiles.len() >= WARMUP_TILES {
            MAX_LIMIT
        } else {
            WARMUP_LIMIT
        }
    }

    pub fn cached_projection_lines(&mut self, key: &TileKey) -> Option<&[Vec<(f32, f32)>]> {
        self.projection_cache.get_mut(key).map(|entry| {
            entry.last_used = Instant::now();
            entry.normalized_lines.as_slice()
        })
    }

    pub fn cache_projection_from_geometry(&mut self, key: TileKey, geometry: &TileGeometry) {
        let bounds = match self.current_projection_bounds {
            Some(bounds) => bounds,
            None => return,
        };
        let lat_span = bounds.lat_span();
        let lng_span = bounds.lng_span();
        if lat_span <= 0.0 || lng_span <= 0.0 {
            return;
        }

        const TOLERANCE: f32 = 0.002;
        let mut normalized_lines = Vec::new();
        for line in &geometry.lines {
            let mut projected = Vec::new();
            let mut last_point: Option<(f32, f32)> = None;
            for &(lat, lng) in line {
                let mut x = ((lng - bounds.lng_min) / lng_span) as f32;
                let mut y = ((bounds.lat_max - lat) / lat_span) as f32;
                x = x.clamp(0.0, 1.0);
                y = y.clamp(0.0, 1.0);
                let point = (x, y);
                if let Some(last) = last_point {
                    if (point.0 - last.0).abs() < TOLERANCE && (point.1 - last.1).abs() < TOLERANCE
                    {
                        continue;
                    }
                }
                projected.push(point);
                last_point = Some(point);
            }
            if projected.len() >= 2 {
                normalized_lines.push(projected);
            }
        }
        if normalized_lines.is_empty() {
            return;
        }
        self.projection_cache.insert(
            key,
            CachedProjection {
                normalized_lines,
                last_used: Instant::now(),
            },
        );
    }

    pub fn evict_stale_projections(&mut self) {
        let now = Instant::now();
        let ttl = Duration::from_secs(5);
        self.projection_cache
            .retain(|_, entry| now.duration_since(entry.last_used) <= ttl);
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
            let limit = self.current_inflight_limit();
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
                let result = fetch_tile(&url, key);
                let _ = sender.send(result);
            });
        }
    }

    pub fn tile(&self, key: &TileKey) -> Option<&TileGeometry> {
        self.tiles.get(key)
    }
}

fn fetch_tile(url: &str, key: TileKey) -> TileResult {
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            return TileResult {
                key,
                geometry: None,
                error: Some(err.to_string()),
            }
        }
    };
    let response = match client.get(url).send() {
        Ok(response) => response,
        Err(err) => {
            return TileResult {
                key,
                geometry: None,
                error: Some(err.to_string()),
            }
        }
    };
    if !response.status().is_success() {
        return TileResult {
            key,
            geometry: None,
            error: Some(format!("status {}", response.status())),
        };
    }
    let bytes = match response.bytes() {
        Ok(bytes) => bytes,
        Err(err) => {
            return TileResult {
                key,
                geometry: None,
                error: Some(err.to_string()),
            }
        }
    };
    match decode_tile_geometry(key, bytes.to_vec()) {
        Ok(geometry) => TileResult {
            key,
            geometry: Some(geometry),
            error: None,
        },
        Err(err) => TileResult {
            key,
            geometry: None,
            error: Some(err),
        },
    }
}

fn decode_tile_geometry(key: TileKey, data: Vec<u8>) -> Result<TileGeometry, String> {
    use prost::Message;

    let tile = VectorTile::decode(data.as_slice()).map_err(|err| err.to_string())?;
    let layer = match tile.layers.iter().find(|layer| layer.name == "speeds") {
        Some(layer) => layer,
        None => return Ok(TileGeometry { lines: Vec::new() }),
    };
    let extent = if layer.extent == 0 {
        4096.0
    } else {
        layer.extent as f64
    };
    let mut lines = Vec::new();
    for feature in &layer.features {
        if feature.r#type != GeomType::Linestring as i32 {
            continue;
        }
        let tile_lines = decode_line_strings(&feature.geometry);
        for line in tile_lines {
            let points = line
                .into_iter()
                .map(|(x, y)| tile_point_to_lat_lng(key, x as f64, y as f64, extent))
                .collect();
            lines.push(points);
        }
    }
    Ok(TileGeometry { lines })
}

fn decode_line_strings(geometry: &[u32]) -> Vec<Vec<(i32, i32)>> {
    let mut lines: Vec<Vec<(i32, i32)>> = Vec::new();
    let mut cursor = 0usize;
    let mut x = 0i32;
    let mut y = 0i32;
    while cursor < geometry.len() {
        let command = geometry[cursor];
        cursor += 1;
        let id = command & 0x7;
        let count = command >> 3;
        match id {
            1 => {
                for _ in 0..count {
                    if cursor + 1 >= geometry.len() {
                        break;
                    }
                    x += decode_zigzag(geometry[cursor]);
                    y += decode_zigzag(geometry[cursor + 1]);
                    cursor += 2;
                    lines.push(vec![(x, y)]);
                }
            }
            2 => {
                for _ in 0..count {
                    if cursor + 1 >= geometry.len() {
                        break;
                    }
                    x += decode_zigzag(geometry[cursor]);
                    y += decode_zigzag(geometry[cursor + 1]);
                    cursor += 2;
                    if let Some(current) = lines.last_mut() {
                        current.push((x, y));
                    }
                }
            }
            7 => {}
            _ => break,
        }
    }
    lines
}

fn decode_zigzag(value: u32) -> i32 {
    ((value >> 1) as i32) ^ (-((value & 1) as i32))
}

fn tile_point_to_lat_lng(key: TileKey, x: f64, y: f64, extent: f64) -> (f64, f64) {
    let n = (1u32 << key.z) as f64;
    let gx = (key.x as f64 + (x / extent)) / n;
    let gy = (key.y as f64 + (y / extent)) / n;
    let lng = gx * 360.0 - 180.0;
    let lat = (std::f64::consts::PI * (1.0 - 2.0 * gy))
        .sinh()
        .atan()
        .to_degrees();
    (lat, lng)
}
