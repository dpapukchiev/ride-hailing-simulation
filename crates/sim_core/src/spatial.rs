use h3o::{CellIndex, Resolution};

#[derive(Debug, Clone, Copy)]
pub struct GeoIndex {
    resolution: Resolution,
}

impl GeoIndex {
    pub fn new(resolution: Resolution) -> Self {
        Self { resolution }
    }

    pub fn resolution(&self) -> Resolution {
        self.resolution
    }

    pub fn grid_disk(&self, origin: CellIndex, k: u32) -> Vec<CellIndex> {
        debug_assert_eq!(
            origin.resolution(),
            self.resolution,
            "origin resolution must match GeoIndex resolution"
        );
        origin.grid_disk::<Vec<_>>(k)
    }
}

impl Default for GeoIndex {
    fn default() -> Self {
        Self {
            resolution: Resolution::Nine,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_disk_returns_neighbors_within_k() {
        let geo = GeoIndex::new(Resolution::Ten);
        let origin = CellIndex::try_from(0x8a1fb46622dffff).expect("valid cell");
        let cells = geo.grid_disk(origin, 1);

        assert!(cells.contains(&origin));
        assert!(!cells.is_empty());
        for cell in cells {
            let distance = origin.grid_distance(cell).expect("grid distance");
            assert!(distance <= 1);
        }
    }
}
