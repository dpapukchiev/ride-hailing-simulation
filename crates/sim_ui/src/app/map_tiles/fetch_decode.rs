use std::time::Duration;

use super::types::{TileGeometry, TileKey, TileResult};

pub(crate) fn fetch_tile(url: &str, key: TileKey) -> TileResult {
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
