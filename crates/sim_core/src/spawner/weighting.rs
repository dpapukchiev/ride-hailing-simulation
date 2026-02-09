use bevy_ecs::prelude::Resource;
use h3o::{CellIndex, LatLng, Resolution};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct WeightedCell {
    pub cell: CellIndex,
    pub weight: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub enum SpawnWeightingKind {
    #[default]
    Uniform,
    BerlinHotspots,
}

#[derive(Debug, Resource)]
pub struct SpawnWeighting {
    pub rider_cells: Vec<WeightedCell>,
    pub driver_cells: Vec<WeightedCell>,
    rider_cumulative: Vec<f64>,
    driver_cumulative: Vec<f64>,
}

impl SpawnWeighting {
    pub fn uniform() -> Self {
        Self {
            rider_cells: Vec::new(),
            driver_cells: Vec::new(),
            rider_cumulative: Vec::new(),
            driver_cumulative: Vec::new(),
        }
    }

    pub fn berlin_hotspots() -> Self {
        let rider_hotspots = vec![
            (52.520, 13.405, 3.0),
            (52.521, 13.413, 2.5),
            (52.525, 13.369, 2.5),
            (52.497, 13.391, 2.0),
            (52.516, 13.454, 2.0),
            (52.538, 13.424, 1.8),
            (52.507, 13.304, 1.5),
            (52.484, 13.353, 1.5),
            (52.477, 13.442, 1.5),
            (52.549, 13.359, 1.2),
            (52.509, 13.376, 2.0),
            (52.502, 13.326, 1.8),
            (52.554, 13.292, 1.0),
            (52.473, 13.401, 1.2),
            (52.535, 13.197, 0.8),
        ];

        let driver_hotspots = vec![
            (52.525, 13.369, 3.0),
            (52.521, 13.413, 2.5),
            (52.507, 13.332, 2.0),
            (52.510, 13.434, 2.0),
            (52.475, 13.365, 1.8),
            (52.520, 13.405, 2.0),
            (52.509, 13.376, 1.8),
            (52.520, 13.387, 1.5),
            (52.549, 13.388, 1.2),
            (52.534, 13.198, 0.8),
        ];

        fn cells_from_coords(data: &[(f64, f64, f64)]) -> (Vec<WeightedCell>, Vec<f64>) {
            let mut cells = Vec::new();
            let mut cumulative = Vec::new();
            let mut total = 0.0;
            for &(lat, lng, weight) in data {
                if let Ok(ll) = LatLng::new(lat, lng) {
                    let cell = ll.to_cell(Resolution::Nine);
                    cells.push(WeightedCell { cell, weight });
                    total += weight;
                    cumulative.push(total);
                }
            }
            (cells, cumulative)
        }

        let (rider_cells, rider_cumulative) = cells_from_coords(&rider_hotspots);
        let (driver_cells, driver_cumulative) = cells_from_coords(&driver_hotspots);

        Self {
            rider_cells,
            driver_cells,
            rider_cumulative,
            driver_cumulative,
        }
    }

    pub fn from_kind(kind: &SpawnWeightingKind) -> Self {
        match kind {
            SpawnWeightingKind::Uniform => Self::uniform(),
            SpawnWeightingKind::BerlinHotspots => Self::berlin_hotspots(),
        }
    }

    pub fn sample_rider_cell<R: rand::Rng>(&self, rng: &mut R) -> Option<CellIndex> {
        self.sample_from(&self.rider_cells, &self.rider_cumulative, rng)
    }

    pub fn sample_driver_cell<R: rand::Rng>(&self, rng: &mut R) -> Option<CellIndex> {
        self.sample_from(&self.driver_cells, &self.driver_cumulative, rng)
    }

    fn sample_from<R: rand::Rng>(
        &self,
        cells: &[WeightedCell],
        cumulative: &[f64],
        rng: &mut R,
    ) -> Option<CellIndex> {
        if cells.is_empty() || cumulative.is_empty() {
            return None;
        }
        let total = *cumulative.last()?;
        if total <= 0.0 {
            return None;
        }
        let r: f64 = rng.gen_range(0.0..total);
        let idx = cumulative.partition_point(|&w| w <= r).min(cells.len() - 1);
        Some(cells[idx].cell)
    }
}
