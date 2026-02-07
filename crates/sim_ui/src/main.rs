mod app;
mod ui;

use eframe::egui;
use egui_plot::{Line, Plot};
use std::time::{Duration, Instant};

use sim_core::matching::MatchingAlgorithmResource;
use sim_core::telemetry::SimSnapshots;

use crate::app::SimUiApp;
use crate::ui::controls::render_control_panel;
use crate::ui::rendering::{
    draw_agent, draw_grid, project_cell, render_map_legend, render_metrics_legend,
    render_trip_table_all, MapBounds,
};
use crate::ui::utils::{
    chart_color_abandoned_quote, chart_color_active_trips, chart_color_cancelled_riders,
    chart_color_cancelled_trips, chart_color_completed_trips, chart_color_idle_drivers,
    chart_color_waiting_riders, driver_color, format_datetime_from_unix_ms, rider_color,
};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_maximized(true),
        ..Default::default()
    };
    eframe::run_native(
        "Ride-Hailing Simulation",
        options,
        Box::new(|cc| {
            // Scale UI to 80% to fit better on screen
            cc.egui_ctx.set_pixels_per_point(0.8);
            Ok(Box::new(SimUiApp::new()))
        }),
    )
}

impl eframe::App for SimUiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update matching algorithm resource if it changed (works even during simulation)
        if self.matching_algorithm_changed {
            let new_algorithm = self.create_matching_algorithm();
            if let Some(mut resource) = self.world.get_resource_mut::<MatchingAlgorithmResource>() {
                *resource = new_algorithm;
            }
            self.matching_algorithm_changed = false;
        }

        if self.auto_run && self.started {
            let now = Instant::now();
            let last = self.last_frame_instant.unwrap_or(now);
            let mut delta_secs = now.saturating_duration_since(last).as_secs_f64();
            if delta_secs <= 0.0 {
                delta_secs = 0.016;
            }
            self.last_frame_instant = Some(now);
            self.sim_budget_ms += delta_secs * 1000.0 * self.speed_multiplier;
            self.advance_by_budget(self.sim_budget_ms);
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        egui::TopBottomPanel::top("controls").show(ctx, |ui| {
            render_control_panel(ui, self);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let (
                latest_snapshot,
                active_trips_points,
                waiting_riders_points,
                idle_drivers_points,
                cancelled_riders_points,
                abandoned_quote_points,
                completed_trips_points,
                cancelled_trips_points,
            ) = if let Some(snapshots) = self.world.get_resource::<SimSnapshots>() {
                let latest = snapshots.snapshots.back().cloned();
                let sim_epoch_ms = self
                    .world
                    .get_resource::<sim_core::clock::SimulationClock>()
                    .map(|clock| clock.epoch_ms())
                    .unwrap_or(0);
                let mut active = Vec::new();
                let mut waiting = Vec::new();
                let mut idle = Vec::new();
                let mut cancelled_riders = Vec::new();
                let mut abandoned_quote = Vec::new();
                let mut completed_trips = Vec::new();
                let mut cancelled_trips = Vec::new();
                for snapshot in snapshots.snapshots.iter() {
                    let real_ms = sim_epoch_ms.saturating_add(snapshot.timestamp_ms as i64);
                    let t = real_ms as f64 / 1000.0;
                    let active_trips =
                        (snapshot.counts.trips_en_route + snapshot.counts.trips_on_trip) as f64;
                    active.push([t, active_trips]);
                    waiting.push([t, snapshot.counts.riders_waiting as f64]);
                    idle.push([t, snapshot.counts.drivers_idle as f64]);
                    cancelled_riders.push([t, snapshot.counts.riders_cancelled_total as f64]);
                    abandoned_quote.push([t, snapshot.counts.riders_abandoned_quote_total as f64]);
                    completed_trips.push([t, snapshot.counts.trips_completed as f64]);
                    cancelled_trips.push([t, snapshot.counts.trips_cancelled as f64]);
                }
                (
                    latest,
                    active,
                    waiting,
                    idle,
                    cancelled_riders,
                    abandoned_quote,
                    completed_trips,
                    cancelled_trips,
                )
            } else {
                return;
            };

            egui::CollapsingHeader::new("Map")
                .default_open(true)
                .show(ui, |ui| {
                    ui.group(|ui| {
                        ui.heading("Map Legend");
                        render_map_legend(ui);

                        let map_height = 560.0;
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

                        if let Some(snapshot) = latest_snapshot.as_ref() {
                            let params = self.current_params();
                            let bounds = MapBounds::new(
                                params.lat_min,
                                params.lat_max,
                                params.lng_min,
                                params.lng_max,
                            );
                            if self.grid_enabled {
                                let spacing_km = (self.map_size_km / 10.0).clamp(0.5, 10.0);
                                draw_grid(&painter, &bounds, map_rect, spacing_km);
                            }
                            if self.show_riders {
                                for rider in &snapshot.riders {
                                    if rider.state != sim_core::ecs::RiderState::InTransit {
                                        if let Some(pos) = project_cell(rider.cell, &bounds, map_rect) {
                                            draw_agent(&painter, pos, "R", rider_color(rider.state, rider.matched_driver));
                                        }
                                    }
                                }
                            }
                            if self.show_drivers {
                                let current_time = snapshot.timestamp_ms;
                                for driver in &snapshot.drivers {
                                    // Filter off-duty drivers if hide_off_duty_drivers is enabled
                                    if self.hide_off_duty_drivers && driver.state == sim_core::ecs::DriverState::OffDuty {
                                        continue;
                                    }
                                    if let Some(pos) = project_cell(driver.cell, &bounds, map_rect) {
                                        let mut label = String::from("D");
                                        if driver.state == sim_core::ecs::DriverState::OnTrip {
                                            label.push_str("(R)");
                                        }
                                        if self.show_driver_stats {
                                            if let (Some(earnings), Some(target)) = (driver.daily_earnings, driver.daily_earnings_target) {
                                                label.push_str(&format!("[{:.0}/{:.0}]", earnings, target));
                                            }
                                            if let (Some(session_start), Some(fatigue_threshold)) = (
                                                driver.session_start_time_ms,
                                                driver.fatigue_threshold_ms,
                                            ) {
                                                let session_duration_ms = current_time.saturating_sub(session_start);
                                                let current_hours = (session_duration_ms as f64 / (60.0 * 60.0 * 1000.0)).round() as u32;
                                                let max_hours = (fatigue_threshold as f64 / (60.0 * 60.0 * 1000.0)).round() as u32;
                                                label.push_str(&format!("[{}/{}h]", current_hours, max_hours));
                                            }
                                        }
                                        draw_agent(&painter, pos, &label, driver_color(driver.state));
                                    }
                                }
                            }
                        }
                    });
                });

            egui::CollapsingHeader::new("Metrics")
                .default_open(false)
                .show(ui, |ui| {
                    ui.group(|ui| {
                        ui.heading("Metrics Legend");
                        render_metrics_legend(ui);
                        Plot::new("active_trips_plot")
                            .height(340.0)
                            .x_axis_formatter(|mark, _range| {
                                format_datetime_from_unix_ms((mark.value * 1000.0) as u64)
                            })
                            .show(ui, |plot_ui| {
                                plot_ui.line(
                                    Line::new("Active trips", active_trips_points.clone())
                                        .color(chart_color_active_trips()),
                                );
                                plot_ui.line(
                                    Line::new("Waiting riders", waiting_riders_points.clone())
                                        .color(chart_color_waiting_riders()),
                                );
                                plot_ui.line(
                                    Line::new("Idle drivers", idle_drivers_points.clone())
                                        .color(chart_color_idle_drivers()),
                                );
                                plot_ui.line(
                                    Line::new("Cancelled riders", cancelled_riders_points.clone())
                                        .color(chart_color_cancelled_riders()),
                                );
                                plot_ui.line(
                                    Line::new("Abandoned (quote)", abandoned_quote_points.clone())
                                        .color(chart_color_abandoned_quote()),
                                );
                                plot_ui.line(
                                    Line::new("Completed trips", completed_trips_points.clone())
                                        .color(chart_color_completed_trips()),
                                );
                                plot_ui.line(
                                    Line::new("Cancelled trips", cancelled_trips_points.clone())
                                        .color(chart_color_cancelled_trips()),
                                );
                            });
                    });
                });

            egui::CollapsingHeader::new("Trips")
                .default_open(false)
                .show(ui, |ui| {
                    if let Some(snapshot) = latest_snapshot.as_ref() {
                        let sim_epoch_ms = self
                            .world
                            .get_resource::<sim_core::clock::SimulationClock>()
                            .map(|clock| clock.epoch_ms())
                            .unwrap_or(0);
                        render_trip_table_all(ui, snapshot.trips.as_slice(), sim_epoch_ms);
                    } else {
                        ui.label("Waiting for first snapshot...");
                    }
                });
        });
    }
}
