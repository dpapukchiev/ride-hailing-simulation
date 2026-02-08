//! Rendering functions for map, charts, and tables.

use eframe::egui::{self, Align2, Color32, FontId, Vec2};
use h3o::{CellIndex, LatLng};

use sim_core::telemetry::{DriverState, RiderState, TripSnapshot, TripState};

use crate::app::TileKey;
use crate::ui::utils::{
    chart_color_abandoned_quote, chart_color_active_trips, chart_color_cancelled_riders,
    chart_color_cancelled_trips, chart_color_completed_trips, chart_color_idle_drivers,
    chart_color_waiting_riders, driver_color, format_distance_km, format_optional_sim_datetime,
    format_sim_datetime_from_ms, format_trip_distance_km, rider_color,
};

/// Geographic bounds for map projection.
pub struct MapBounds {
    pub lat_min: f64,
    pub lat_max: f64,
    pub lng_min: f64,
    pub lng_max: f64,
}

impl MapBounds {
    pub fn new(lat_min: f64, lat_max: f64, lng_min: f64, lng_max: f64) -> Self {
        Self {
            lat_min,
            lat_max,
            lng_min,
            lng_max,
        }
    }
}

/// Project an H3 cell to screen coordinates.
pub fn project_cell(cell: CellIndex, bounds: &MapBounds, rect: egui::Rect) -> Option<egui::Pos2> {
    let center: LatLng = cell.into();
    let lat = center.lat();
    let lng = center.lng();

    if bounds.lat_max <= bounds.lat_min || bounds.lng_max <= bounds.lng_min {
        return None;
    }

    let x = (lng - bounds.lng_min) / (bounds.lng_max - bounds.lng_min);
    let y = (bounds.lat_max - lat) / (bounds.lat_max - bounds.lat_min);
    if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
        return None;
    }

    let px = rect.left() + rect.width() * x as f32;
    let py = rect.top() + rect.height() * y as f32;
    Some(egui::pos2(px, py))
}

pub fn project_lat_lng_unclamped(
    lat: f64,
    lng: f64,
    bounds: &MapBounds,
    rect: egui::Rect,
) -> Option<egui::Pos2> {
    if bounds.lat_max <= bounds.lat_min || bounds.lng_max <= bounds.lng_min {
        return None;
    }
    let x = (lng - bounds.lng_min) / (bounds.lng_max - bounds.lng_min);
    let y = (bounds.lat_max - lat) / (bounds.lat_max - bounds.lat_min);
    let px = rect.left() + rect.width() * x as f32;
    let py = rect.top() + rect.height() * y as f32;
    Some(egui::pos2(px, py))
}

fn clamp_lat(lat: f64) -> f64 {
    lat.clamp(-85.05112878, 85.05112878)
}

fn lon_to_x(lon: f64, zoom: u8) -> f64 {
    let n = (1u32 << zoom) as f64;
    ((lon + 180.0) / 360.0) * n
}

fn lat_to_y(lat: f64, zoom: u8) -> f64 {
    let lat = clamp_lat(lat).to_radians();
    let n = (1u32 << zoom) as f64;
    let y = (1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / std::f64::consts::PI) * 0.5;
    y * n
}

fn tile_count(bounds: &MapBounds, zoom: u8) -> usize {
    let x_min = lon_to_x(bounds.lng_min, zoom).floor() as i64;
    let x_max = lon_to_x(bounds.lng_max, zoom).floor() as i64;
    let y_min = lat_to_y(bounds.lat_max, zoom).floor() as i64;
    let y_max = lat_to_y(bounds.lat_min, zoom).floor() as i64;
    let width = (x_max - x_min + 1).max(0) as usize;
    let height = (y_max - y_min + 1).max(0) as usize;
    width.saturating_mul(height)
}

pub fn choose_tile_zoom(bounds: &MapBounds) -> u8 {
    let mut chosen = 12u8;
    for zoom in 12u8..=16u8 {
        let count = tile_count(bounds, zoom);
        if count <= 12 {
            chosen = zoom;
        }
    }
    chosen
}

pub fn tiles_for_bounds(bounds: &MapBounds, zoom: u8) -> Vec<TileKey> {
    let x_min = lon_to_x(bounds.lng_min, zoom).floor() as i64;
    let x_max = lon_to_x(bounds.lng_max, zoom).floor() as i64;
    let y_min = lat_to_y(bounds.lat_max, zoom).floor() as i64;
    let y_max = lat_to_y(bounds.lat_min, zoom).floor() as i64;
    let max_index = (1u32 << zoom).saturating_sub(1) as i64;
    let x_start = x_min.clamp(0, max_index) as u32;
    let x_end = x_max.clamp(0, max_index) as u32;
    let y_start = y_min.clamp(0, max_index) as u32;
    let y_end = y_max.clamp(0, max_index) as u32;
    let mut tiles = Vec::new();
    for x in x_start..=x_end {
        for y in y_start..=y_end {
            tiles.push(TileKey { z: zoom, x, y });
        }
    }
    tiles
}

/// Draw an agent (rider or driver) on the map.
pub fn draw_agent(painter: &egui::Painter, pos: egui::Pos2, label: &str, color: Color32) {
    painter.circle_filled(pos, 4.0, color);
    painter.text(
        pos + Vec2::new(6.0, -6.0),
        Align2::LEFT_TOP,
        label,
        FontId::monospace(8.5),
        color,
    );
}

/// Draw a grid overlay on the map.
pub fn draw_grid(painter: &egui::Painter, bounds: &MapBounds, rect: egui::Rect, spacing_km: f64) {
    if spacing_km <= 0.0 {
        return;
    }

    let lat_mid = (bounds.lat_min + bounds.lat_max) * 0.5;
    let meters_per_deg_lat = 111_320.0;
    let meters_per_deg_lng = 111_320.0 * lat_mid.to_radians().cos().max(0.1);
    let spacing_m = spacing_km * 1000.0;
    let lat_step = spacing_m / meters_per_deg_lat;
    let lng_step = spacing_m / meters_per_deg_lng;

    let stroke = egui::Stroke::new(1.0, Color32::from_gray(40));

    let mut lat = bounds.lat_min;
    while lat <= bounds.lat_max {
        let y = (bounds.lat_max - lat) / (bounds.lat_max - bounds.lat_min);
        let py = rect.top() + rect.height() * y as f32;
        painter.line_segment(
            [egui::pos2(rect.left(), py), egui::pos2(rect.right(), py)],
            stroke,
        );
        lat += lat_step;
    }

    let mut lng = bounds.lng_min;
    while lng <= bounds.lng_max {
        let x = (lng - bounds.lng_min) / (bounds.lng_max - bounds.lng_min);
        let px = rect.left() + rect.width() * x as f32;
        painter.line_segment(
            [egui::pos2(px, rect.top()), egui::pos2(px, rect.bottom())],
            stroke,
        );
        lng += lng_step;
    }
}

/// Render a legend item (color swatch + label).
fn legend_item(ui: &mut egui::Ui, color: Color32, label: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 14.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 2.0, color);
        ui.label(label);
    });
}

/// Render the map legend showing rider and driver states.
pub fn render_map_legend(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label("Riders:");
        legend_item(ui, rider_color(RiderState::Browsing, None), "Browsing");
        legend_item(
            ui,
            rider_color(RiderState::Waiting, None),
            "Waiting for match",
        );
        legend_item(ui, rider_color_waiting_for_pickup(), "Waiting for pickup");
        legend_item(ui, rider_color(RiderState::InTransit, None), "In transit");
        legend_item(ui, rider_color(RiderState::Completed, None), "Completed");
        legend_item(ui, rider_color(RiderState::Cancelled, None), "Cancelled");
    });
    ui.horizontal(|ui| {
        ui.label("Drivers:");
        legend_item(ui, driver_color(DriverState::Idle), "Idle");
        legend_item(ui, driver_color(DriverState::Evaluating), "Evaluating");
        legend_item(ui, driver_color(DriverState::EnRoute), "En route");
        legend_item(ui, driver_color(DriverState::OnTrip), "On trip");
        legend_item(ui, driver_color(DriverState::OffDuty), "Off duty");
    });
}

/// Render the metrics chart legend.
pub fn render_metrics_legend(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label("Charts:");
        legend_item(ui, chart_color_active_trips(), "Active trips");
        legend_item(ui, chart_color_waiting_riders(), "Waiting riders");
        legend_item(ui, chart_color_idle_drivers(), "Idle drivers");
        legend_item(ui, chart_color_cancelled_riders(), "Cancelled riders");
        legend_item(ui, chart_color_abandoned_quote(), "Abandoned (quote)");
        legend_item(ui, chart_color_completed_trips(), "Completed trips");
        legend_item(ui, chart_color_cancelled_trips(), "Cancelled trips");
    });
}

/// Helper color for riders waiting for pickup (used in legend).
fn rider_color_waiting_for_pickup() -> Color32 {
    Color32::from_rgb(255, 100, 0)
}

/// Get the last updated timestamp for a trip snapshot.
fn last_updated_time(trip: &TripSnapshot) -> u64 {
    let mut max_time = trip.requested_at.max(trip.matched_at);
    if let Some(pickup_at) = trip.pickup_at {
        max_time = max_time.max(pickup_at);
    }
    if let Some(dropoff_at) = trip.dropoff_at {
        max_time = max_time.max(dropoff_at);
    }
    if let Some(cancelled_at) = trip.cancelled_at {
        max_time = max_time.max(cancelled_at);
    }
    max_time
}

/// Render the trip state as a string label.
fn trip_state_label(state: TripState) -> &'static str {
    match state {
        TripState::EnRoute => "EnRoute",
        TripState::OnTrip => "OnTrip",
        TripState::Completed => "Completed",
        TripState::Cancelled => "Cancelled",
    }
}

/// Render the complete trip table with all trips.
pub fn render_trip_table_all(ui: &mut egui::Ui, trips: &[TripSnapshot], sim_epoch_ms: i64) {
    ui.group(|ui| {
        let available_width = ui.available_width();
        ui.set_min_width(available_width);
        ui.heading("Trips");
        ui.label("Live table updates as trip state changes.");

        let mut rows: Vec<&TripSnapshot> = trips.iter().collect();
        // Sort by last updated time (most recent first)
        rows.sort_by(|a, b| {
            let a_last = last_updated_time(a);
            let b_last = last_updated_time(b);
            b_last.cmp(&a_last) // Descending order (newest first)
        });

        render_trip_table_section(
            ui,
            "trip_table_all",
            &rows,
            available_width,
            280.0,
            sim_epoch_ms,
        );
    });
}

/// Render a section of the trip table.
fn render_trip_table_section(
    ui: &mut egui::Ui,
    table_id: &str,
    rows: &[&TripSnapshot],
    available_width: f32,
    max_height: f32,
    sim_epoch_ms: i64,
) {
    egui::ScrollArea::vertical()
        .id_salt(format!("{}_scroll", table_id))
        .auto_shrink([false, true])
        .max_height(max_height)
        .show(ui, |ui| {
            ui.set_min_width(available_width);
            egui::Grid::new(table_id)
                .min_col_width(available_width / 11.0)
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Trip");
                    ui.label("Rider");
                    ui.label("Driver");
                    ui.label("State");
                    ui.label("Pickup km (accept)");
                    ui.label("Distance km");
                    ui.label("Requested");
                    ui.label("Matched");
                    ui.label("Started");
                    ui.label("Completed");
                    ui.label("Cancelled");
                    ui.end_row();

                    for trip in rows {
                        ui.label(trip.entity.to_bits().to_string());
                        ui.label(trip.rider.to_bits().to_string());
                        ui.label(trip.driver.to_bits().to_string());
                        ui.label(trip_state_label(trip.state));
                        ui.label(format_distance_km(trip.pickup_distance_km_at_accept));
                        ui.label(format_trip_distance_km(trip.pickup_cell, trip.dropoff_cell));
                        ui.label(format_sim_datetime_from_ms(sim_epoch_ms, trip.requested_at));
                        ui.label(format_sim_datetime_from_ms(sim_epoch_ms, trip.matched_at));
                        ui.label(format_optional_sim_datetime(sim_epoch_ms, trip.pickup_at));
                        ui.label(format_optional_sim_datetime(sim_epoch_ms, trip.dropoff_at));
                        ui.label(format_optional_sim_datetime(
                            sim_epoch_ms,
                            trip.cancelled_at,
                        ));
                        ui.end_row();
                    }
                });
        });
}
