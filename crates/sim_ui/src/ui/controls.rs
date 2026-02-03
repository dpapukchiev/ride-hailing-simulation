//! Control panel UI for simulation parameters and actions.

use eframe::egui;

use crate::app::{MatchingAlgorithmType, SimUiApp};
use crate::ui::utils::{datetime_from_unix_ms, format_datetime_from_unix_ms, format_hms_from_ms, now_unix_ms};

/// Render the top control panel with simulation controls and parameters.
pub fn render_control_panel(ui: &mut egui::Ui, app: &mut SimUiApp) {
    ui.horizontal(|ui| {
        let can_start = !app.started;
        if ui
            .add_enabled(can_start, egui::Button::new("Start"))
            .clicked()
        {
            app.start_simulation();
        }
        if ui.button(if app.auto_run { "Pause" } else { "Run" }).clicked() {
            if app.started {
                app.auto_run = !app.auto_run;
                if app.auto_run {
                    app.last_frame_instant = Some(std::time::Instant::now());
                }
            }
        }
        if ui.button("Step").clicked() {
            if !app.started {
                app.start_simulation();
            }
            app.run_steps(1);
        }
        if ui.button("Step 100").clicked() {
            if !app.started {
                app.start_simulation();
            }
            app.run_steps(100);
        }
        if ui.button("Run to end").clicked() {
            if !app.started {
                app.start_simulation();
            }
            app.auto_run = false;
            app.run_until_done();
        }
        if ui.button("Reset").clicked() {
            app.reset();
        }
    });

    ui.horizontal(|ui| {
        ui.label("Clock speed");
        egui::ComboBox::from_id_salt("clock_speed")
            .selected_text(format!("{}x", app.speed_multiplier as u32))
            .show_ui(ui, |ui| {
                for speed in [10.0, 20.0, 50.0, 100.0, 200.0] {
                    ui.selectable_value(
                        &mut app.speed_multiplier,
                        speed,
                        format!("{}x", speed as u32),
                    );
                }
            });
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut app.show_riders, "Riders");
        ui.checkbox(&mut app.show_drivers, "Drivers");
        ui.checkbox(&mut app.show_driver_stats, "Driver stats (earnings/fatigue)");
        ui.checkbox(&mut app.grid_enabled, "Grid");
        ui.label(format!("Steps executed: {}", app.steps_executed));
    });

    let (sim_now_ms, sim_epoch_ms) = app
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
            render_scenario_parameters(ui, app);
        });
}

/// Render scenario parameter inputs in an eight-column layout to use horizontal space.
fn render_scenario_parameters(ui: &mut egui::Ui, app: &mut SimUiApp) {
    let can_edit = !app.started;

    ui.columns(8, |columns| {
        // Col 1: Riders
        columns[0].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("Riders");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.num_riders).range(1..=10_000),
                );
            });
        });

        // Col 2: Drivers
        columns[1].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("Drivers");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.num_drivers).range(1..=10_000),
                );
            });
        });

        // Col 3: Initial riders
        columns[2].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("Initial riders");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.initial_rider_count).range(0..=10_000),
                );
            });
        });

        // Col 4: Initial drivers
        columns[3].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("Initial drivers");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.initial_driver_count).range(0..=10_000),
                );
            });
        });

        // Col 5: Rider spread, Driver spread
        columns[4].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("Rider spread (h)");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.request_window_hours).range(1..=24),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Driver spread (h)");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.driver_spread_hours).range(1..=24),
                );
            });
        });

        // Col 6: Match radius, Map size, Trip length
        columns[5].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("Match radius (km)");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.match_radius_km)
                        .range(0.0..=20.0)
                        .speed(0.1),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Map size (km)");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.map_size_km)
                        .range(1.0..=200.0)
                        .speed(1.0),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Trip (km)");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.min_trip_km)
                        .range(0.1..=100.0)
                        .speed(0.1),
                );
                ui.label("–");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.max_trip_km)
                        .range(0.1..=200.0)
                        .speed(0.1),
                );
            });
        });

        // Col 7: Cancel wait, Seed
        columns[6].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("Cancel wait (m)");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.rider_cancel_min_mins)
                        .range(1..=600),
                );
                ui.label("–");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.rider_cancel_max_mins)
                        .range(1..=600),
                );
            });
            ui.horizontal(|ui| {
                ui.add_enabled(can_edit, egui::Checkbox::new(&mut app.seed_enabled, "Seed"));
                ui.add_enabled(
                    can_edit && app.seed_enabled,
                    egui::DragValue::new(&mut app.seed_value).range(0..=u64::MAX),
                );
            });
        });

        // Col 8: Start time, Matching algorithm
        columns[7].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("Start (UTC)");
            });
            ui.horizontal(|ui| {
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.start_year).range(1970..=2100).suffix(" Y"),
                );
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.start_month).range(1..=12).suffix(" M"),
                );
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.start_day).range(1..=31).suffix(" D"),
                );
            });
            ui.horizontal(|ui| {
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.start_hour).range(0..=23).suffix(" H"),
                );
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.start_minute).range(0..=59).suffix(" m"),
                );
                if ui.add_enabled(can_edit, egui::Button::new("Now")).clicked() {
                    let (year, month, day, hour, minute) = datetime_from_unix_ms(now_unix_ms());
                    app.start_year = year;
                    app.start_month = month;
                    app.start_day = day;
                    app.start_hour = hour;
                    app.start_minute = minute;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Matching");
                let old_algorithm = app.matching_algorithm;
                egui::ComboBox::from_id_salt("matching_algorithm")
                    .selected_text(match app.matching_algorithm {
                        MatchingAlgorithmType::Simple => "Simple",
                        MatchingAlgorithmType::CostBased => "Cost-based",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut app.matching_algorithm,
                            MatchingAlgorithmType::Simple,
                            "Simple (first match)",
                        );
                        ui.selectable_value(
                            &mut app.matching_algorithm,
                            MatchingAlgorithmType::CostBased,
                            "Cost-based (best match)",
                        );
                    });
                if app.matching_algorithm != old_algorithm {
                    app.matching_algorithm_changed = true;
                }
            });
        });
    });
}
