use bevy_ecs::prelude::World;
use eframe::egui::{self, Align2, Color32, FontId, Vec2};
use egui_plot::{Line, Plot};
use h3o::{CellIndex, LatLng};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sim_core::ecs::{DriverState, RiderState, TripState};
use sim_core::matching::MatchingAlgorithmResource;
use sim_core::runner::{run_next_event, simulation_schedule};
use sim_core::scenario::{build_scenario, create_cost_based_matching, create_simple_matching, RiderCancelConfig, ScenarioParams};
use sim_core::telemetry::{SimSnapshotConfig, SimSnapshots, TripSnapshot};

const H3_RES9_CELL_WIDTH_KM: f64 = 0.24;
const METERS_PER_DEG_LAT: f64 = 111_320.0;

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

struct SimUiApp {
    world: World,
    schedule: bevy_ecs::schedule::Schedule,
    steps_executed: usize,
    auto_run: bool,
    started: bool,
    snapshot_interval_ms: u64,
    speed_multiplier: f64,
    sim_budget_ms: f64,
    last_frame_instant: Option<Instant>,
    num_riders: usize,
    num_drivers: usize,
    initial_rider_count: usize,
    initial_driver_count: usize,
    request_window_hours: u64,
    driver_spread_hours: u64,
    match_radius_km: f64,
    min_trip_km: f64,
    max_trip_km: f64,
    seed_enabled: bool,
    seed_value: u64,
    grid_enabled: bool,
    map_size_km: f64,
    rider_cancel_min_mins: u64,
    rider_cancel_max_mins: u64,
    show_riders: bool,
    show_drivers: bool,
    show_driver_stats: bool,
    matching_algorithm: MatchingAlgorithmType,
    matching_algorithm_changed: bool, // Track if algorithm was changed in UI
    start_year: i32,
    start_month: u32,
    start_day: u32,
    start_hour: u32,
    start_minute: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatchingAlgorithmType {
    Simple,
    CostBased,
}

impl MatchingAlgorithmType {
    fn create_matching_algorithm(&self) -> MatchingAlgorithmResource {
        match self {
            MatchingAlgorithmType::Simple => create_simple_matching(),
            MatchingAlgorithmType::CostBased => create_cost_based_matching(0.1),
        }
    }
}

impl SimUiApp {
    fn new() -> Self {
        let num_riders = 80;
        let num_drivers = 50;
        let initial_rider_count = 0;
        let initial_driver_count = 10;
        let request_window_hours = 21;
        let driver_spread_hours = 21;
        let match_radius_km = 11.0;
        let min_trip_km = 1.0;
        let max_trip_km = 25.0;
        let map_size_km = 25.0;
        let rider_cancel_min_mins = 10;
        let rider_cancel_max_mins = 40;
        let seed_enabled = true;
        let seed_value = 123;
        let matching_algorithm = MatchingAlgorithmType::CostBased;
        
        // Default start time: 2026-02-03 20:12:00 UTC
        let year = 2026;
        let month = 2;
        let day = 3;
        let hour = 20;
        let minute = 12;
        let start_epoch_ms = datetime_to_unix_ms(year, month, day, hour, minute);

        let mut params = ScenarioParams {
            num_riders,
            num_drivers,
            ..Default::default()
        }
        .with_request_window_hours(request_window_hours)
        .with_match_radius(km_to_cells(match_radius_km))
        .with_trip_duration_cells(km_to_cells(min_trip_km), km_to_cells(max_trip_km))
        .with_epoch_ms(start_epoch_ms);
        if seed_enabled {
            params = params.with_seed(seed_value);
        }

        let mut world = World::new();
        build_scenario(&mut world, params);
        // Override the default algorithm with the selected one
        world.insert_resource(matching_algorithm.create_matching_algorithm());
        // Clock epoch is already set in build_scenario from params.epoch_ms
        sim_core::runner::initialize_simulation(&mut world);
        let schedule = simulation_schedule();

        Self {
            world,
            schedule,
            steps_executed: 0,
            auto_run: false,
            started: false,
            snapshot_interval_ms: 1000,
            speed_multiplier: 200.0,
            sim_budget_ms: 0.0,
            last_frame_instant: None,
            num_riders,
            num_drivers,
            initial_rider_count,
            initial_driver_count,
            request_window_hours,
            driver_spread_hours,
            match_radius_km,
            min_trip_km,
            max_trip_km,
            seed_enabled,
            seed_value,
            grid_enabled: false,
            map_size_km,
            rider_cancel_min_mins,
            rider_cancel_max_mins,
            show_riders: true,
            show_drivers: true,
            show_driver_stats: true,
            matching_algorithm,
            matching_algorithm_changed: false,
            start_year: year,
            start_month: month,
            start_day: day,
            start_hour: hour,
            start_minute: minute,
        }
    }

    fn reset(&mut self) {
        let mut world = World::new();
        build_scenario(&mut world, self.current_params());
        // Override the default algorithm with the selected one
        world.insert_resource(self.create_matching_algorithm());
        apply_cancel_config(
            &mut world,
            self.rider_cancel_min_mins,
            self.rider_cancel_max_mins,
        );
        // Clock epoch is already set in build_scenario from params.epoch_ms
        apply_snapshot_interval(&mut world, self.snapshot_interval_ms);
        sim_core::runner::initialize_simulation(&mut world);
        self.world = world;
        // Note: Schedule creation is fast (just adding systems), so recreating is fine
        // The main performance bottleneck was in build_scenario (grid_disk generation)
        self.schedule = simulation_schedule();
        self.steps_executed = 0;
        self.auto_run = false;
        self.started = false;
        self.sim_budget_ms = 0.0;
        self.last_frame_instant = None;
        self.matching_algorithm_changed = false;
    }

    fn start_simulation(&mut self) {
        let mut world = World::new();
        build_scenario(&mut world, self.current_params());
        // Override the default algorithm with the selected one
        world.insert_resource(self.create_matching_algorithm());
        apply_cancel_config(
            &mut world,
            self.rider_cancel_min_mins,
            self.rider_cancel_max_mins,
        );
        // Clock epoch is already set in build_scenario from params.epoch_ms
        apply_snapshot_interval(&mut world, self.snapshot_interval_ms);
        sim_core::runner::initialize_simulation(&mut world);
        self.world = world;
        self.schedule = simulation_schedule();
        self.steps_executed = 0;
        self.started = true;
        self.auto_run = true;
        self.sim_budget_ms = 0.0;
        self.last_frame_instant = Some(Instant::now());
    }

    fn create_matching_algorithm(&self) -> MatchingAlgorithmResource {
        match self.matching_algorithm {
            MatchingAlgorithmType::Simple => create_simple_matching(),
            MatchingAlgorithmType::CostBased => create_cost_based_matching(0.1),
        }
    }

    fn current_params(&self) -> ScenarioParams {
        let start_epoch_ms = datetime_to_unix_ms(self.start_year, self.start_month, self.start_day, self.start_hour, self.start_minute);
        let mut params = ScenarioParams {
            num_riders: self.num_riders,
            num_drivers: self.num_drivers,
            initial_rider_count: self.initial_rider_count,
            initial_driver_count: self.initial_driver_count,
            ..Default::default()
        }
        .with_request_window_hours(self.request_window_hours)
        .with_driver_spread_hours(self.driver_spread_hours)
        .with_match_radius(km_to_cells(self.match_radius_km))
        .with_trip_duration_cells(km_to_cells(self.min_trip_km), km_to_cells(self.max_trip_km))
        .with_epoch_ms(start_epoch_ms);
        let (lat_min, lat_max, lng_min, lng_max) = bounds_from_km(self.map_size_km);
        params.lat_min = lat_min;
        params.lat_max = lat_max;
        params.lng_min = lng_min;
        params.lng_max = lng_max;

        if self.seed_enabled {
            params = params.with_seed(self.seed_value);
        }
        params
    }

    fn run_steps(&mut self, steps: usize) {
        for _ in 0..steps {
            if !run_next_event(&mut self.world, &mut self.schedule) {
                break;
            }
            self.steps_executed += 1;
        }
    }

    fn run_until_done(&mut self) {
        loop {
            if !run_next_event(&mut self.world, &mut self.schedule) {
                break;
            }
            self.steps_executed += 1;
        }
    }

    fn advance_by_budget(&mut self, budget_ms: f64) {
        let mut remaining = budget_ms.max(0.0);
        while let Some((next_ts, sim_now)) = self
            .world
            .get_resource::<sim_core::clock::SimulationClock>()
            .and_then(|clock| Some((clock.next_event_time()?, clock.now())))
        {
            if next_ts <= sim_now {
                if !run_next_event(&mut self.world, &mut self.schedule) {
                    break;
                }
                self.steps_executed += 1;
                continue;
            }

            let gap = (next_ts - sim_now) as f64;
            if gap > remaining {
                break;
            }
            if !run_next_event(&mut self.world, &mut self.schedule) {
                break;
            }
            self.steps_executed += 1;
            remaining -= gap;
        }
        self.sim_budget_ms = remaining;
    }
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
            ui.horizontal(|ui| {
                let can_start = !self.started;
                if ui
                    .add_enabled(can_start, egui::Button::new("Start"))
                    .clicked()
                {
                    self.start_simulation();
                }
                if ui.button(if self.auto_run { "Pause" } else { "Run" }).clicked() {
                    if self.started {
                        self.auto_run = !self.auto_run;
                        if self.auto_run {
                            self.last_frame_instant = Some(Instant::now());
                        }
                    }
                }
                if ui.button("Step").clicked() {
                    if !self.started {
                        self.start_simulation();
                    }
                    self.run_steps(1);
                }
                if ui.button("Step 100").clicked() {
                    if !self.started {
                        self.start_simulation();
                    }
                    self.run_steps(100);
                }
                if ui.button("Run to end").clicked() {
                    if !self.started {
                        self.start_simulation();
                    }
                    self.auto_run = false;
                    self.run_until_done();
                }
                if ui.button("Reset").clicked() {
                    self.reset();
                }
            });

            ui.horizontal(|ui| {
                ui.label("Clock speed");
                egui::ComboBox::from_id_salt("clock_speed")
                    .selected_text(format!("{}x", self.speed_multiplier as u32))
                    .show_ui(ui, |ui| {
                        for speed in [10.0, 20.0, 50.0, 100.0, 200.0] {
                            ui.selectable_value(
                                &mut self.speed_multiplier,
                                speed,
                                format!("{}x", speed as u32),
                            );
                        }
                    });
            });

            ui.horizontal(|ui| {
                ui.checkbox(&mut self.show_riders, "Riders");
                ui.checkbox(&mut self.show_drivers, "Drivers");
                ui.checkbox(&mut self.show_driver_stats, "Driver stats (earnings/fatigue)");
                ui.checkbox(&mut self.grid_enabled, "Grid");
                ui.label(format!("Steps executed: {}", self.steps_executed));
            });

            let (sim_now_ms, sim_epoch_ms) = self
                .world
                .get_resource::<sim_core::clock::SimulationClock>()
                .map(|clock| (clock.now(), clock.epoch_ms()))
                .unwrap_or((0, 0));
            let sim_real_ms = sim_epoch_ms.saturating_add(sim_now_ms as i64).max(0) as u64;
            ui.horizontal(|ui| {
                ui.label(format!(
                    "Sim time: {}",
                    format_hms_from_ms(sim_now_ms)
                ));
                ui.label(format!(
                    "Sim datetime (UTC): {}",
                    format_datetime_from_unix_ms(sim_real_ms)
                ));
                ui.label(format!(
                    "Wall clock (UTC): {}",
                    format_datetime_from_unix_ms(now_unix_ms())
                ));
            });

            egui::CollapsingHeader::new("Scenario parameters")
                .default_open(true)
                .show(ui, |ui| {
                    let can_edit = !self.started;
                    ui.horizontal(|ui| {
                        ui.label("Riders");
                        ui.add_enabled(
                            can_edit,
                            egui::DragValue::new(&mut self.num_riders).range(1..=10_000),
                        );
                        ui.label("Drivers");
                        ui.add_enabled(
                            can_edit,
                            egui::DragValue::new(&mut self.num_drivers).range(1..=10_000),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Initial riders");
                        ui.add_enabled(
                            can_edit,
                            egui::DragValue::new(&mut self.initial_rider_count).range(0..=10_000),
                        );
                        ui.label("Initial drivers");
                        ui.add_enabled(
                            can_edit,
                            egui::DragValue::new(&mut self.initial_driver_count).range(0..=10_000),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Rider demand spread (hours)");
                        ui.add_enabled(
                            can_edit,
                            egui::DragValue::new(&mut self.request_window_hours).range(1..=24),
                        );
                        ui.label("Driver spread (hours)");
                        ui.add_enabled(
                            can_edit,
                            egui::DragValue::new(&mut self.driver_spread_hours).range(1..=24),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Match radius (km)");
                        ui.add_enabled(
                            can_edit,
                            egui::DragValue::new(&mut self.match_radius_km)
                                .range(0.0..=20.0)
                                .speed(0.1),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Map size (km)");
                        ui.add_enabled(
                            can_edit,
                            egui::DragValue::new(&mut self.map_size_km)
                                .range(1.0..=200.0)
                                .speed(1.0),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Trip length (km)");
                        ui.add_enabled(
                            can_edit,
                            egui::DragValue::new(&mut self.min_trip_km)
                                .range(0.1..=100.0)
                                .speed(0.1),
                        );
                        ui.label("to");
                        ui.add_enabled(
                            can_edit,
                            egui::DragValue::new(&mut self.max_trip_km)
                                .range(0.1..=200.0)
                                .speed(0.1),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Cancel wait (mins)");
                        ui.add_enabled(
                            can_edit,
                            egui::DragValue::new(&mut self.rider_cancel_min_mins)
                                .range(1..=600),
                        );
                        ui.label("to");
                        ui.add_enabled(
                            can_edit,
                            egui::DragValue::new(&mut self.rider_cancel_max_mins)
                                .range(1..=600),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.add_enabled(can_edit, egui::Checkbox::new(&mut self.seed_enabled, "Seed"));
                        ui.add_enabled(
                            can_edit && self.seed_enabled,
                            egui::DragValue::new(&mut self.seed_value).range(0..=u64::MAX),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Start time (UTC):");
                        ui.add_enabled(
                            can_edit,
                            egui::DragValue::new(&mut self.start_year).range(1970..=2100).suffix(" Y"),
                        );
                        ui.add_enabled(
                            can_edit,
                            egui::DragValue::new(&mut self.start_month).range(1..=12).suffix(" M"),
                        );
                        ui.add_enabled(
                            can_edit,
                            egui::DragValue::new(&mut self.start_day).range(1..=31).suffix(" D"),
                        );
                        ui.add_enabled(
                            can_edit,
                            egui::DragValue::new(&mut self.start_hour).range(0..=23).suffix(" H"),
                        );
                        ui.add_enabled(
                            can_edit,
                            egui::DragValue::new(&mut self.start_minute).range(0..=59).suffix(" m"),
                        );
                        if ui.add_enabled(can_edit, egui::Button::new("Now")).clicked() {
                            let (year, month, day, hour, minute) = datetime_from_unix_ms(now_unix_ms());
                            self.start_year = year;
                            self.start_month = month;
                            self.start_day = day;
                            self.start_hour = hour;
                            self.start_minute = minute;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Matching algorithm");
                        let old_algorithm = self.matching_algorithm;
                        egui::ComboBox::from_id_salt("matching_algorithm")
                            .selected_text(match self.matching_algorithm {
                                MatchingAlgorithmType::Simple => "Simple (first match)",
                                MatchingAlgorithmType::CostBased => "Cost-based (best match)",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.matching_algorithm,
                                    MatchingAlgorithmType::Simple,
                                    "Simple (first match)",
                                );
                                ui.selectable_value(
                                    &mut self.matching_algorithm,
                                    MatchingAlgorithmType::CostBased,
                                    "Cost-based (best match)",
                                );
                            });
                        // Track if algorithm changed (works even during simulation)
                        if self.matching_algorithm != old_algorithm {
                            self.matching_algorithm_changed = true;
                        }
                    });
                });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let (
                latest_snapshot,
                active_trips_points,
                waiting_riders_points,
                idle_drivers_points,
                cancelled_riders_points,
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
                    completed_trips.push([t, snapshot.counts.trips_completed as f64]);
                    cancelled_trips.push([t, snapshot.counts.trips_cancelled as f64]);
                }
                (
                    latest,
                    active,
                    waiting,
                    idle,
                    cancelled_riders,
                    completed_trips,
                    cancelled_trips,
                )
            } else {
                return;
            };

            ui.group(|ui| {
                ui.heading("Map Legend");
                render_map_legend(ui);

                let map_height = 280.0;
                let map_size = Vec2::new(ui.available_width(), map_height);
                let (map_rect, _) = ui.allocate_exact_size(map_size, egui::Sense::hover());
                let painter = ui.painter_at(map_rect);

                painter.rect_filled(map_rect, 0.0, Color32::from_gray(20));
                painter.rect_stroke(
                    map_rect,
                    0.0,
                    egui::Stroke::new(1.0, Color32::from_gray(60)),
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
                            // Hide riders that are in transit (they're with the driver)
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
                            if let Some(pos) = project_cell(driver.cell, &bounds, map_rect) {
                                // Build label with state, earnings, and fatigue info (compact format)
                                let mut label = String::from("D");
                                
                                // Show "(R)" for drivers on trip (with rider)
                                if driver.state == sim_core::ecs::DriverState::OnTrip {
                                    label.push_str("(R)");
                                }
                                
                                // Add earnings and fatigue info only if toggle is enabled
                                if self.show_driver_stats {
                                    // Add earnings info: [earned/target] without dollar signs
                                    if let (Some(earnings), Some(target)) = (driver.daily_earnings, driver.daily_earnings_target) {
                                        label.push_str(&format!("[{:.0}/{:.0}]", earnings, target));
                                    }
                                    
                                    // Add fatigue info: [current/max]h (integers for compactness)
                                    if let (Some(session_start), Some(fatigue_threshold)) = (
                                        driver.session_start_time_ms,
                                        driver.fatigue_threshold_ms
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

            ui.add_space(8.0);

            ui.group(|ui| {
                ui.heading("Metrics Legend");
                render_metrics_legend(ui);
                Plot::new("active_trips_plot")
                    .height(220.0)
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
                            Line::new("Completed trips", completed_trips_points.clone())
                                .color(chart_color_completed_trips()),
                        );
                        plot_ui.line(
                            Line::new("Cancelled trips", cancelled_trips_points.clone())
                                .color(chart_color_cancelled_trips()),
                        );
                    });
            });

            ui.add_space(8.0);

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
    }
}

struct MapBounds {
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
}

impl MapBounds {
    fn new(lat_min: f64, lat_max: f64, lng_min: f64, lng_max: f64) -> Self {
        Self {
            lat_min,
            lat_max,
            lng_min,
            lng_max,
        }
    }
}

fn project_cell(cell: CellIndex, bounds: &MapBounds, rect: egui::Rect) -> Option<egui::Pos2> {
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

fn draw_agent(painter: &egui::Painter, pos: egui::Pos2, label: &str, color: Color32) {
    painter.circle_filled(pos, 4.0, color);
    painter.text(
        pos + Vec2::new(6.0, -6.0),
        Align2::LEFT_TOP,
        label,
        FontId::monospace(8.5),
        color,
    );
}

fn draw_grid(painter: &egui::Painter, bounds: &MapBounds, rect: egui::Rect, spacing_km: f64) {
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
        painter.line_segment([egui::pos2(rect.left(), py), egui::pos2(rect.right(), py)], stroke);
        lat += lat_step;
    }

    let mut lng = bounds.lng_min;
    while lng <= bounds.lng_max {
        let x = (lng - bounds.lng_min) / (bounds.lng_max - bounds.lng_min);
        let px = rect.left() + rect.width() * x as f32;
        painter.line_segment([egui::pos2(px, rect.top()), egui::pos2(px, rect.bottom())], stroke);
        lng += lng_step;
    }
}

fn legend_item(ui: &mut egui::Ui, color: Color32, label: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 14.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 2.0, color);
        ui.label(label);
    });
}

fn render_map_legend(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label("Riders:");
        legend_item(ui, rider_color(RiderState::Browsing, None), "Browsing");
        legend_item(ui, rider_color(RiderState::Waiting, None), "Waiting for match");
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

fn render_metrics_legend(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label("Charts:");
        legend_item(ui, chart_color_active_trips(), "Active trips");
        legend_item(ui, chart_color_waiting_riders(), "Waiting riders");
        legend_item(ui, chart_color_idle_drivers(), "Idle drivers");
        legend_item(ui, chart_color_cancelled_riders(), "Cancelled riders");
        legend_item(ui, chart_color_completed_trips(), "Completed trips");
        legend_item(ui, chart_color_cancelled_trips(), "Cancelled trips");
    });
}

fn rider_color(state: RiderState, matched_driver: Option<bevy_ecs::prelude::Entity>) -> Color32 {
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

fn rider_color_waiting_for_pickup() -> Color32 {
    // Helper for legend - represents waiting for pickup color
    Color32::from_rgb(255, 100, 0)
}

fn driver_color(state: DriverState) -> Color32 {
    match state {
        DriverState::Idle => Color32::from_rgb(0, 200, 120),
        DriverState::Evaluating => Color32::from_rgb(255, 200, 0),
        DriverState::EnRoute => Color32::from_rgb(255, 140, 0),
        DriverState::OnTrip => Color32::from_rgb(80, 140, 255),
        DriverState::OffDuty => Color32::from_gray(100),
    }
}

fn chart_color_active_trips() -> Color32 {
    Color32::from_rgb(80, 140, 255)
}

fn chart_color_waiting_riders() -> Color32 {
    Color32::from_rgb(255, 140, 0)
}

fn chart_color_idle_drivers() -> Color32 {
    Color32::from_rgb(0, 200, 120)
}

fn chart_color_cancelled_riders() -> Color32 {
    Color32::from_rgb(200, 80, 80)
}

fn chart_color_completed_trips() -> Color32 {
    Color32::from_rgb(160, 200, 80)
}

fn chart_color_cancelled_trips() -> Color32 {
    Color32::from_rgb(160, 80, 200)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis() as u64
}

fn format_hms_from_ms(ms: u64) -> String {
    let total_secs = ms / 1000;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

fn format_datetime_from_unix_ms(ms: u64) -> String {
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

fn datetime_from_unix_ms(ms: u64) -> (i32, u32, u32, u32, u32) {
    let total_secs = (ms / 1000) as i64;
    let days = total_secs / 86_400;
    let day_secs = total_secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hours = (day_secs / 3600) as u32;
    let minutes = ((day_secs % 3600) / 60) as u32;
    (year, month, day, hours, minutes)
}

fn datetime_to_unix_ms(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> i64 {
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

fn apply_snapshot_interval(world: &mut World, interval_ms: u64) {
    if let Some(mut config) = world.get_resource_mut::<SimSnapshotConfig>() {
        config.interval_ms = interval_ms;
    }
}

fn apply_cancel_config(world: &mut World, min_mins: u64, max_mins: u64) {
    if let Some(mut config) = world.get_resource_mut::<RiderCancelConfig>() {
        let min_secs = min_mins.saturating_mul(60);
        let max_secs = max_mins.saturating_mul(60);
        // Preserve the seed when updating bounds
        let seed = config.seed;
        config.min_wait_secs = min_secs;
        config.max_wait_secs = max_secs.max(min_secs);
        config.seed = seed;
    }
}

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

fn render_trip_table_all(ui: &mut egui::Ui, trips: &[TripSnapshot], sim_epoch_ms: i64) {
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
                        ui.label(format_optional_sim_datetime(sim_epoch_ms, trip.cancelled_at));
                        ui.end_row();
                    }
                });
        });
}

fn trip_state_label(state: TripState) -> &'static str {
    match state {
        TripState::EnRoute => "EnRoute",
        TripState::OnTrip => "OnTrip",
        TripState::Completed => "Completed",
        TripState::Cancelled => "Cancelled",
    }
}

fn km_to_cells(km: f64) -> u32 {
    if km <= 0.0 {
        return 0;
    }
    (km / H3_RES9_CELL_WIDTH_KM).ceil().max(1.0) as u32
}

fn bounds_from_km(size_km: f64) -> (f64, f64, f64, f64) {
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

fn format_sim_datetime_from_ms(sim_epoch_ms: i64, sim_ms: u64) -> String {
    let real_ms = sim_epoch_ms.saturating_add(sim_ms as i64).max(0) as u64;
    format_datetime_from_unix_ms(real_ms)
}

fn format_optional_sim_datetime(sim_epoch_ms: i64, sim_ms: Option<u64>) -> String {
    sim_ms
        .map(|value| format_sim_datetime_from_ms(sim_epoch_ms, value))
        .unwrap_or_else(|| "-".to_string())
}

fn format_trip_distance_km(pickup: CellIndex, dropoff: CellIndex) -> String {
    let distance_km = distance_km_between_cells(pickup, dropoff);
    format_distance_km(distance_km)
}

fn format_distance_km(distance_km: f64) -> String {
    format!("{:.2} km", distance_km)
}

fn distance_km_between_cells(a: CellIndex, b: CellIndex) -> f64 {
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
