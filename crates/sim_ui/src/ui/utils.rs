//! Utility functions for the UI: formatting, colors, conversions.

use eframe::egui::Color32;
use h3o::{CellIndex, LatLng};
use sim_core::ecs::{DriverState, RiderState};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::ui::constants::{H3_RES9_CELL_WIDTH_KM, METERS_PER_DEG_LAT};
use sim_core::scenario::ScenarioParams;

pub fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis() as u64
}

pub fn format_hms_from_ms(ms: u64) -> String {
    let total_secs = ms / 1000;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

pub fn format_datetime_from_unix_ms(ms: u64) -> String {
    let total_secs = (ms / 1000) as i64;
    let days = total_secs / 86_400;
    let day_secs = total_secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hours = day_secs / 3600;
    let minutes = (day_secs % 3600) / 60;
    let seconds = day_secs % 60;
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        year, month, day, hours, minutes, seconds
    )
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year as i32, m as u32, d as u32)
}

pub fn datetime_from_unix_ms(ms: u64) -> (i32, u32, u32, u32, u32) {
    let total_secs = (ms / 1000) as i64;
    let days = total_secs / 86_400;
    let day_secs = total_secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hours = (day_secs / 3600) as u32;
    let minutes = ((day_secs % 3600) / 60) as u32;
    (year, month, day, hours, minutes)
}

pub fn datetime_to_unix_ms(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> i64 {
    // Convert date to days since Unix epoch
    // Algorithm from https://howardhinnant.github.io/date_algorithms.html
    let y = year as i64;
    let m = month as i64;
    let d = day as i64;
    
    // Adjust for month
    let adjusted_m = if m <= 2 { m + 12 } else { m };
    let adjusted_y = if m <= 2 { y - 1 } else { y };
    
    // Calculate days since epoch (1970-01-01)
    let era = (if adjusted_y >= 0 { adjusted_y } else { adjusted_y - 399 }) / 400;
    let yoe = adjusted_y - era * 400;
    let doy = (153 * (adjusted_m - 3) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    
    // Add time components
    let total_secs = days * 86400 + hour as i64 * 3600 + minute as i64 * 60;
    total_secs * 1000 // Convert to milliseconds
}

pub fn format_sim_datetime_from_ms(sim_epoch_ms: i64, sim_ms: u64) -> String {
    let real_ms = sim_epoch_ms.saturating_add(sim_ms as i64).max(0) as u64;
    format_datetime_from_unix_ms(real_ms)
}

pub fn format_optional_sim_datetime(sim_epoch_ms: i64, sim_ms: Option<u64>) -> String {
    sim_ms
        .map(|value| format_sim_datetime_from_ms(sim_epoch_ms, value))
        .unwrap_or_else(|| "-".to_string())
}

pub fn format_distance_km(distance_km: f64) -> String {
    format!("{:.2} km", distance_km)
}

pub fn format_trip_distance_km(pickup: CellIndex, dropoff: CellIndex) -> String {
    let distance_km = distance_km_between_cells(pickup, dropoff);
    format_distance_km(distance_km)
}

pub fn distance_km_between_cells(a: CellIndex, b: CellIndex) -> f64 {
    let a: LatLng = a.into();
    let b: LatLng = b.into();
    let (lat1, lon1) = (a.lat().to_radians(), a.lng().to_radians());
    let (lat2, lon2) = (b.lat().to_radians(), b.lng().to_radians());
    let dlat = lat2 - lat1;
    let dlon = lon2 - lon1;
    let sin_dlat = (dlat * 0.5).sin();
    let sin_dlon = (dlon * 0.5).sin();
    let h = sin_dlat * sin_dlat + lat1.cos() * lat2.cos() * sin_dlon * sin_dlon;
    let c = 2.0 * h.sqrt().atan2((1.0 - h).sqrt());
    6371.0 * c
}

pub fn km_to_cells(km: f64) -> u32 {
    if km <= 0.0 {
        return 0;
    }
    (km / H3_RES9_CELL_WIDTH_KM).ceil().max(1.0) as u32
}

pub fn bounds_from_km(size_km: f64) -> (f64, f64, f64, f64) {
    let clamped_km = size_km.max(1.0);
    let half_km = clamped_km * 0.5;
    let defaults = ScenarioParams::default();
    let center_lat = 0.5 * (defaults.lat_min + defaults.lat_max);
    let center_lng = 0.5 * (defaults.lng_min + defaults.lng_max);
    let lat_delta = (half_km * 1000.0) / METERS_PER_DEG_LAT;
    let lng_delta =
        (half_km * 1000.0) / (METERS_PER_DEG_LAT * center_lat.to_radians().cos().max(0.1));
    (
        center_lat - lat_delta,
        center_lat + lat_delta,
        center_lng - lng_delta,
        center_lng + lng_delta,
    )
}

pub fn rider_color(state: RiderState, matched_driver: Option<bevy_ecs::prelude::Entity>) -> Color32 {
    match state {
        RiderState::Browsing => Color32::from_rgb(120, 180, 255),
        RiderState::Waiting => {
            // Differentiate between waiting for match vs waiting for pickup
            if matched_driver.is_some() {
                // Waiting for pickup (driver matched, en route)
                Color32::from_rgb(255, 100, 0) // Darker orange/red
            } else {
                // Waiting for match (no driver yet)
                Color32::from_rgb(255, 200, 0) // Brighter yellow/orange
            }
        }
        RiderState::InTransit => Color32::from_rgb(0, 200, 120),
        RiderState::Completed => Color32::from_gray(140),
        RiderState::Cancelled => Color32::from_rgb(200, 80, 80),
    }
}

pub fn driver_color(state: DriverState) -> Color32 {
    match state {
        DriverState::Idle => Color32::from_rgb(0, 200, 120),
        DriverState::Evaluating => Color32::from_rgb(255, 200, 0),
        DriverState::EnRoute => Color32::from_rgb(255, 140, 0),
        DriverState::OnTrip => Color32::from_rgb(80, 140, 255),
        DriverState::OffDuty => Color32::from_gray(100),
    }
}

pub fn chart_color_active_trips() -> Color32 {
    Color32::from_rgb(80, 140, 255)
}

pub fn chart_color_waiting_riders() -> Color32 {
    Color32::from_rgb(255, 140, 0)
}

pub fn chart_color_idle_drivers() -> Color32 {
    Color32::from_rgb(0, 200, 120)
}

pub fn chart_color_cancelled_riders() -> Color32 {
    Color32::from_rgb(200, 80, 80)
}

pub fn chart_color_completed_trips() -> Color32 {
    Color32::from_rgb(160, 200, 80)
}

pub fn chart_color_cancelled_trips() -> Color32 {
    Color32::from_rgb(160, 80, 200)
}
