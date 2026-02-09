use std::time::Instant;

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

#[derive(Debug, Clone)]
pub struct TileGeometry {
    pub lines: Vec<Vec<(f64, f64)>>,
}

#[derive(Clone, Copy)]
pub(crate) struct ProjectionBounds {
    pub lat_min: f64,
    pub lat_max: f64,
    pub lng_min: f64,
    pub lng_max: f64,
}

impl ProjectionBounds {
    pub(crate) fn new(lat_min: f64, lat_max: f64, lng_min: f64, lng_max: f64) -> Option<Self> {
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

    pub(crate) fn lat_span(&self) -> f64 {
        self.lat_max - self.lat_min
    }

    pub(crate) fn lng_span(&self) -> f64 {
        self.lng_max - self.lng_min
    }
}

pub(crate) struct CachedProjection {
    pub normalized_lines: Vec<Vec<(f32, f32)>>,
    pub last_used: Instant,
}

pub(crate) struct TileResult {
    pub key: TileKey,
    pub geometry: Option<TileGeometry>,
    pub error: Option<String>,
}
