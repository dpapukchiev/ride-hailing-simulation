//! Control panel UI for simulation parameters and actions.

use eframe::egui;

use sim_core::telemetry::{CompletedTripRecord, SimSnapshots, SimTelemetry};

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

    egui::CollapsingHeader::new("Run outcomes")
        .default_open(true)
        .show(ui, |ui| {
            render_run_outcomes(ui, app);
        });
}

/// Render run outcome counters and optional completed-trip timing stats from SimTelemetry.
fn render_run_outcomes(ui: &mut egui::Ui, app: &SimUiApp) {
    let telemetry = match app.world.get_resource::<SimTelemetry>() {
        Some(t) => t,
        None => {
            ui.label("—");
            return;
        }
    };

    // Outcome counters: 3 columns
    let total_resolved = telemetry.riders_completed_total
        .saturating_add(telemetry.riders_cancelled_total)
        .saturating_add(telemetry.riders_abandoned_quote_total);
    let conversion_pct = if total_resolved > 0 {
        (telemetry.riders_completed_total as f64 / total_resolved as f64) * 100.0
    } else {
        0.0
    };

    ui.columns(3, |columns| {
        columns[0].vertical(|ui| {
            ui.label("Riders completed");
            ui.label(telemetry.riders_completed_total.to_string());
            ui.label("Trips completed");
            ui.label(telemetry.completed_trips.len().to_string());
        });
        columns[1].vertical(|ui| {
            ui.label("Riders cancelled");
            ui.label(telemetry.riders_cancelled_total.to_string());
            ui.label("Abandoned (quote)");
            ui.label(telemetry.riders_abandoned_quote_total.to_string());
        });
        columns[2].vertical(|ui| {
            ui.label("Total resolved");
            ui.label(total_resolved.to_string());
            ui.label("Conversion %");
            ui.label(format!("{:.1}%", conversion_pct));
        });
    });

    // Current state (from latest snapshot)
    if let Some(snapshots) = app.world.get_resource::<SimSnapshots>() {
        if let Some(latest) = snapshots.snapshots.back() {
            let c = &latest.counts;
            ui.columns(3, |columns| {
                columns[0].vertical(|ui| {
                    ui.label("Riders now:");
                    ui.horizontal(|ui| {
                        ui.label(format!("browsing {} waiting {} in transit {}", c.riders_browsing, c.riders_waiting, c.riders_in_transit));
                    });
                });
                columns[1].vertical(|ui| {
                    ui.label("Drivers now:");
                    ui.horizontal(|ui| {
                        ui.label(format!("idle {} en route {} on trip {} off duty {}", c.drivers_idle, c.drivers_en_route, c.drivers_on_trip, c.drivers_off_duty));
                    });
                });
                columns[2].vertical(|ui| {
                    ui.label("Trips now:");
                    ui.horizontal(|ui| {
                        ui.label(format!("en route {} on trip {}", c.trips_en_route, c.trips_on_trip));
                    });
                });
            });
        }
    }

    // Timing distribution: 3 columns (time to match | time to pickup | trip duration)
    if !telemetry.completed_trips.is_empty() {
        const PERCENTILES: &[(u8, &str)] = &[(50, "p50"), (90, "p90"), (95, "p95"), (99, "p99")];
        let n = telemetry.completed_trips.len();

        let match_dist = timing_distribution(telemetry.completed_trips.as_slice(), CompletedTripRecord::time_to_match);
        let pickup_dist = timing_distribution(telemetry.completed_trips.as_slice(), CompletedTripRecord::time_to_pickup);
        let trip_dist = timing_distribution(telemetry.completed_trips.as_slice(), CompletedTripRecord::trip_duration);

        ui.columns(3, |columns| {
            columns[0].vertical(|ui| {
                ui.label(format!("Time to match (n={})", n));
                ui.label(format!("min {}  max {}", format_hms_from_ms(match_dist.min), format_hms_from_ms(match_dist.max)));
                for (p, label) in PERCENTILES {
                    if let Some(v) = match_dist.percentile(*p) {
                        ui.label(format!("{} {}", label, format_hms_from_ms(v)));
                    }
                }
                ui.label(format!("avg {}", format_hms_from_ms(match_dist.mean)));
            });
            columns[1].vertical(|ui| {
                ui.label(format!("Time to pickup (n={})", n));
                ui.label(format!("min {}  max {}", format_hms_from_ms(pickup_dist.min), format_hms_from_ms(pickup_dist.max)));
                for (p, label) in PERCENTILES {
                    if let Some(v) = pickup_dist.percentile(*p) {
                        ui.label(format!("{} {}", label, format_hms_from_ms(v)));
                    }
                }
                ui.label(format!("avg {}", format_hms_from_ms(pickup_dist.mean)));
            });
            columns[2].vertical(|ui| {
                ui.label(format!("Trip duration (n={})", n));
                ui.label(format!("min {}  max {}", format_hms_from_ms(trip_dist.min), format_hms_from_ms(trip_dist.max)));
                for (p, label) in PERCENTILES {
                    if let Some(v) = trip_dist.percentile(*p) {
                        ui.label(format!("{} {}", label, format_hms_from_ms(v)));
                    }
                }
                ui.label(format!("avg {}", format_hms_from_ms(trip_dist.mean)));
            });
        });
    }
}

/// Min, mean, max and sorted values for percentile computation. Durations in ms.
struct TimingDistribution {
    min: u64,
    mean: u64,
    max: u64,
    sorted: Vec<u64>,
}

impl TimingDistribution {
    /// Nearest-rank percentile (0–100). Returns None if empty or p out of range.
    fn percentile(&self, p: u8) -> Option<u64> {
        if self.sorted.is_empty() || p > 100 {
            return None;
        }
        let n = self.sorted.len();
        let idx = ((p as usize) * (n - 1)) / 100;
        Some(self.sorted[idx])
    }
}

/// Compute min, mean, max and sorted values for percentiles. Durations in ms.
fn timing_distribution(trips: &[CompletedTripRecord], f: fn(&CompletedTripRecord) -> u64) -> TimingDistribution {
    if trips.is_empty() {
        return TimingDistribution {
            min: 0,
            mean: 0,
            max: 0,
            sorted: Vec::new(),
        };
    }
    let mut values: Vec<u64> = trips.iter().map(f).collect();
    let min = *values.iter().min().unwrap();
    let max = *values.iter().max().unwrap();
    let mean = values.iter().sum::<u64>() / (values.len() as u64);
    values.sort_unstable();
    TimingDistribution {
        min,
        mean,
        max,
        sorted: values,
    }
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
