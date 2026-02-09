use eframe::egui;
use egui_plot::{Line, Plot};

use sim_core::telemetry::SimSnapshots;

use crate::app::{MapSignature, RoutingMode, SimUiApp};
use crate::ui::rendering::{
    choose_tile_zoom, draw_agent, draw_grid, project_lat_lng_unclamped, project_position,
    render_map_legend, render_metrics_legend, render_trip_table_all, tiles_for_bounds, MapBounds,
};
use crate::ui::utils::{
    chart_color_abandoned_quote, chart_color_active_trips, chart_color_cancelled_riders,
    chart_color_cancelled_trips, chart_color_completed_trips, chart_color_idle_drivers,
    chart_color_waiting_riders, driver_color, format_datetime_from_unix_ms, rider_color,
};

struct MetricSeries {
    latest_snapshot: Option<sim_core::telemetry::SimSnapshot>,
    active_trips: Vec<[f64; 2]>,
    waiting_riders: Vec<[f64; 2]>,
    idle_drivers: Vec<[f64; 2]>,
    cancelled_riders: Vec<[f64; 2]>,
    abandoned_quote: Vec<[f64; 2]>,
    completed_trips: Vec<[f64; 2]>,
    cancelled_trips: Vec<[f64; 2]>,
}

pub fn render_dashboard(ui: &mut egui::Ui, app: &mut SimUiApp) {
    app.map_tiles.drain_results();
    app.map_tiles.evict_stale_projections();

    let Some(series) = collect_metric_series(app) else {
        return;
    };

    render_map_panel(ui, app, series.latest_snapshot.as_ref());
    render_metrics_panel(ui, &series);
    render_trips_panel(ui, app, series.latest_snapshot.as_ref());
}

fn collect_metric_series(app: &SimUiApp) -> Option<MetricSeries> {
    let snapshots = app.world.get_resource::<SimSnapshots>()?;
    let latest_snapshot = snapshots.snapshots.back().cloned();
    let sim_epoch_ms = app
        .world
        .get_resource::<sim_core::clock::SimulationClock>()
        .map(|clock| clock.epoch_ms())
        .unwrap_or(0);

    let mut active_trips = Vec::new();
    let mut waiting_riders = Vec::new();
    let mut idle_drivers = Vec::new();
    let mut cancelled_riders = Vec::new();
    let mut abandoned_quote = Vec::new();
    let mut completed_trips = Vec::new();
    let mut cancelled_trips = Vec::new();

    for snapshot in snapshots.snapshots.iter() {
        let real_ms = sim_epoch_ms.saturating_add(snapshot.timestamp_ms as i64);
        let t = real_ms as f64 / 1000.0;
        active_trips.push([
            t,
            (snapshot.counts.trips_en_route + snapshot.counts.trips_on_trip) as f64,
        ]);
        waiting_riders.push([t, snapshot.counts.riders_waiting as f64]);
        idle_drivers.push([t, snapshot.counts.drivers_idle as f64]);
        cancelled_riders.push([t, snapshot.counts.riders_cancelled_total as f64]);
        abandoned_quote.push([t, snapshot.counts.riders_abandoned_quote_total as f64]);
        completed_trips.push([t, snapshot.counts.trips_completed as f64]);
        cancelled_trips.push([t, snapshot.counts.trips_cancelled as f64]);
    }

    Some(MetricSeries {
        latest_snapshot,
        active_trips,
        waiting_riders,
        idle_drivers,
        cancelled_riders,
        abandoned_quote,
        completed_trips,
        cancelled_trips,
    })
}

fn render_map_panel(
    ui: &mut egui::Ui,
    app: &mut SimUiApp,
    latest_snapshot: Option<&sim_core::telemetry::SimSnapshot>,
) {
    egui::CollapsingHeader::new("Map")
        .default_open(true)
        .show(ui, |ui| {
            ui.group(|ui| {
                ui.heading("Map Legend");
                render_map_legend(ui);

                let map_height = 680.0;
                let map_size = egui::Vec2::new(ui.available_width(), map_height);
                let (map_rect, _) = ui.allocate_exact_size(map_size, egui::Sense::hover());
                let painter = ui.painter_at(map_rect);

                painter.rect_filled(map_rect, 0.0, egui::Color32::from_gray(20));
                painter.rect_stroke(
                    map_rect,
                    0.0,
                    egui::Stroke::new(1.0, egui::Color32::from_gray(60)),
                    egui::StrokeKind::Middle,
                );

                if let Some(snapshot) = latest_snapshot {
                    let params = app.current_params();
                    let bounds = MapBounds::new(
                        params.lat_min,
                        params.lat_max,
                        params.lng_min,
                        params.lng_max,
                    );
                    if app.routing_mode == RoutingMode::Osrm {
                        let zoom = choose_tile_zoom(&bounds);
                        let signature = MapSignature {
                            z: zoom,
                            lat_min: (bounds.lat_min * 1_000_000.0).round() as i64,
                            lat_max: (bounds.lat_max * 1_000_000.0).round() as i64,
                            lng_min: (bounds.lng_min * 1_000_000.0).round() as i64,
                            lng_max: (bounds.lng_max * 1_000_000.0).round() as i64,
                        };
                        app.map_tiles.update_signature(signature);
                        let tiles = tiles_for_bounds(&bounds, zoom);
                        if !app.osrm_endpoint.trim().is_empty() {
                            app.map_tiles
                                .request_missing_tiles(&app.osrm_endpoint, tiles.iter().copied());
                        }
                        let road_stroke = egui::Stroke::new(1.0, egui::Color32::from_gray(80));
                        for tile in &tiles {
                            if let Some(lines) = app.map_tiles.cached_projection_lines(tile) {
                                for line in lines {
                                    let points: Vec<egui::Pos2> = line
                                        .iter()
                                        .map(|(nx, ny)| {
                                            egui::pos2(
                                                map_rect.left() + map_rect.width() * *nx,
                                                map_rect.top() + map_rect.height() * *ny,
                                            )
                                        })
                                        .collect();
                                    if points.len() >= 2 {
                                        painter.add(egui::Shape::line(points, road_stroke));
                                    }
                                }
                            } else if let Some(geometry) = app.map_tiles.tile(tile).cloned() {
                                for line in &geometry.lines {
                                    let points: Vec<egui::Pos2> = line
                                        .iter()
                                        .filter_map(|(lat, lng)| {
                                            project_lat_lng_unclamped(*lat, *lng, &bounds, map_rect)
                                        })
                                        .collect();
                                    if points.len() >= 2 {
                                        painter.add(egui::Shape::line(points, road_stroke));
                                    }
                                }
                                app.map_tiles
                                    .cache_projection_from_geometry(*tile, &geometry);
                            }
                        }
                    }
                    if app.grid_enabled {
                        draw_grid(
                            &painter,
                            &bounds,
                            map_rect,
                            (app.map_size_km / 10.0).clamp(0.5, 10.0),
                        );
                    }
                    if app.show_riders {
                        for rider in &snapshot.riders {
                            if let Some(pos) =
                                project_position(rider.cell, rider.geo, &bounds, map_rect)
                            {
                                draw_agent(
                                    &painter,
                                    pos,
                                    "R",
                                    rider_color(rider.state, rider.matched_driver),
                                );
                            }
                        }
                    }
                    if app.show_drivers {
                        let current_time = snapshot.timestamp_ms;
                        for driver in &snapshot.drivers {
                            if app.hide_off_duty_drivers
                                && driver.state == sim_core::telemetry::DriverState::OffDuty
                            {
                                continue;
                            }
                            if let Some(pos) =
                                project_position(driver.cell, driver.geo, &bounds, map_rect)
                            {
                                let mut label = String::from("D");
                                if driver.state == sim_core::telemetry::DriverState::OnTrip {
                                    label.push_str("(R)");
                                }
                                if app.show_driver_stats {
                                    if let (Some(earnings), Some(target)) =
                                        (driver.daily_earnings, driver.daily_earnings_target)
                                    {
                                        label.push_str(&format!("[{:.0}/{:.0}]", earnings, target));
                                    }
                                    if let (Some(session_start), Some(fatigue_threshold)) =
                                        (driver.session_start_time_ms, driver.fatigue_threshold_ms)
                                    {
                                        let end =
                                            driver.session_end_time_ms.unwrap_or(current_time);
                                        let hours = (end.saturating_sub(session_start) as f64
                                            / 3_600_000.0)
                                            .round()
                                            as u32;
                                        let max_hours =
                                            (fatigue_threshold as f64 / 3_600_000.0).round() as u32;
                                        label.push_str(&format!("[{}/{}h]", hours, max_hours));
                                    }
                                }
                                draw_agent(&painter, pos, &label, driver_color(driver.state));
                            }
                        }
                    }
                }
            });
        });
}

fn render_metrics_panel(ui: &mut egui::Ui, series: &MetricSeries) {
    egui::CollapsingHeader::new("Metrics")
        .default_open(false)
        .show(ui, |ui| {
            ui.group(|ui| {
                ui.heading("Metrics Legend");
                render_metrics_legend(ui);
                Plot::new("active_trips_plot")
                    .height(340.0)
                    .x_axis_formatter(|mark, _| {
                        format_datetime_from_unix_ms((mark.value * 1000.0) as u64)
                    })
                    .show(ui, |plot_ui| {
                        plot_ui.line(
                            Line::new("Active trips", series.active_trips.clone())
                                .color(chart_color_active_trips()),
                        );
                        plot_ui.line(
                            Line::new("Waiting riders", series.waiting_riders.clone())
                                .color(chart_color_waiting_riders()),
                        );
                        plot_ui.line(
                            Line::new("Idle drivers", series.idle_drivers.clone())
                                .color(chart_color_idle_drivers()),
                        );
                        plot_ui.line(
                            Line::new("Cancelled riders", series.cancelled_riders.clone())
                                .color(chart_color_cancelled_riders()),
                        );
                        plot_ui.line(
                            Line::new("Abandoned (quote)", series.abandoned_quote.clone())
                                .color(chart_color_abandoned_quote()),
                        );
                        plot_ui.line(
                            Line::new("Completed trips", series.completed_trips.clone())
                                .color(chart_color_completed_trips()),
                        );
                        plot_ui.line(
                            Line::new("Cancelled trips", series.cancelled_trips.clone())
                                .color(chart_color_cancelled_trips()),
                        );
                    });
            });
        });
}

fn render_trips_panel(
    ui: &mut egui::Ui,
    app: &SimUiApp,
    latest_snapshot: Option<&sim_core::telemetry::SimSnapshot>,
) {
    egui::CollapsingHeader::new("Trips")
        .default_open(false)
        .show(ui, |ui| {
            if let Some(snapshot) = latest_snapshot {
                let sim_epoch_ms = app
                    .world
                    .get_resource::<sim_core::clock::SimulationClock>()
                    .map(|clock| clock.epoch_ms())
                    .unwrap_or(0);
                render_trip_table_all(ui, snapshot.trips.as_slice(), sim_epoch_ms);
            } else {
                ui.label("Waiting for first snapshot...");
            }
        });
}
