//! Control panel UI for simulation parameters and actions.

use eframe::egui;

use sim_core::ecs::DriverState;
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
                for speed in [10.0, 20.0, 50.0, 100.0, 200.0, 400.0, 1000.0, 2000.0] {
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
        ui.checkbox(&mut app.hide_off_duty_drivers, "Hide off-duty drivers");
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
        .default_open(false)
        .show(ui, |ui| {
            render_run_outcomes(ui, app);
        });

    egui::CollapsingHeader::new("Fleet")
        .default_open(false)
        .show(ui, |ui| {
            render_fleet(ui, app);
        });
}

/// Render run outcome counters and optional completed-trip timing stats from SimTelemetry.
fn render_run_outcomes(ui: &mut egui::Ui, app: &SimUiApp) {
    const PERCENTILES: &[(u8, &str)] = &[(50, "p50"), (90, "p90"), (95, "p95"), (99, "p99")];
    
    let telemetry = match app.world.get_resource::<SimTelemetry>() {
        Some(t) => t,
        None => {
            ui.label("—");
            return;
        }
    };

    // Outcome counters: 5 columns
    let total_resolved = telemetry.riders_completed_total
        .saturating_add(telemetry.riders_cancelled_total)
        .saturating_add(telemetry.riders_abandoned_quote_total);
    let conversion_pct = if total_resolved > 0 {
        (telemetry.riders_completed_total as f64 / total_resolved as f64) * 100.0
    } else {
        0.0
    };

    ui.columns(6, |columns| {
        columns[0].vertical(|ui| {
            ui.label("Riders completed");
            ui.label(telemetry.riders_completed_total.to_string());
        });
        columns[1].vertical(|ui| {
            ui.label("Riders cancelled");
            ui.label(telemetry.riders_cancelled_total.to_string());
            if telemetry.riders_cancelled_total > 0 {
                let timeout_pct = (telemetry.riders_cancelled_pickup_timeout as f64 / telemetry.riders_cancelled_total as f64) * 100.0;
                ui.add_space(2.0);
                ui.label(format!("  Timeout: {} ({:.1}%)", 
                    telemetry.riders_cancelled_pickup_timeout,
                    timeout_pct));
            }
        });
        columns[2].vertical(|ui| {
            ui.label("Abandoned (quote)");
            ui.label(telemetry.riders_abandoned_quote_total.to_string());
            let total_quote_abandoned = telemetry.riders_abandoned_quote_total;
            if total_quote_abandoned > 0 {
                let price_pct = (telemetry.riders_abandoned_price as f64 / total_quote_abandoned as f64) * 100.0;
                let eta_pct = (telemetry.riders_abandoned_eta as f64 / total_quote_abandoned as f64) * 100.0;
                let stochastic_pct = (telemetry.riders_abandoned_stochastic as f64 / total_quote_abandoned as f64) * 100.0;
                ui.add_space(2.0);
                ui.label(format!("  Price: {} ({:.1}%)", telemetry.riders_abandoned_price, price_pct));
                ui.label(format!("  ETA: {} ({:.1}%)", telemetry.riders_abandoned_eta, eta_pct));
                ui.label(format!("  Stochastic: {} ({:.1}%)", telemetry.riders_abandoned_stochastic, stochastic_pct));
            }
        });
        columns[3].vertical(|ui| {
            ui.label("Trips completed");
            ui.label(telemetry.completed_trips.len().to_string());
        });
        columns[4].vertical(|ui| {
            ui.label("Total resolved");
            ui.label(total_resolved.to_string());
            ui.label("Conversion %");
            ui.label(format!("{:.1}%", conversion_pct));
        });
        columns[5].vertical(|ui| {
            ui.label("Platform revenue");
            ui.label(format!("{:.2}", telemetry.platform_revenue_total));
            ui.label("Total rider pay");
            ui.label(format!("{:.2}", telemetry.total_fares_collected));
            let n = telemetry.completed_trips.len();
            if n > 0 {
                let avg_fare = telemetry.total_fares_collected / n as f64;
                ui.label("Avg fare");
                ui.label(format!("{:.2}", avg_fare));
            }
        });
    });

    // Current state (from latest snapshot): 3 columns (riders, drivers, trips)
    if let Some(snapshots) = app.world.get_resource::<SimSnapshots>() {
        if let Some(latest) = snapshots.snapshots.back() {
            let c = &latest.counts;
            ui.columns(3, |columns| {
                columns[0].vertical(|ui| {
                    ui.label("Riders now:");
                    ui.label(format!("browsing {} waiting {} in transit {}", c.riders_browsing, c.riders_waiting, c.riders_in_transit));
                });
                columns[1].vertical(|ui| {
                    ui.label("Drivers now:");
                    ui.label(format!("idle {} en route {} on trip {} off duty {}", c.drivers_idle, c.drivers_en_route, c.drivers_on_trip, c.drivers_off_duty));
                });
                columns[2].vertical(|ui| {
                    ui.label("Trips now:");
                    ui.label(format!("en route {} on trip {}", c.trips_en_route, c.trips_on_trip));
                });
            });
        }
    }

    ui.add_space(8.0);

    // Timing and fare distributions: 5 columns (time to match | time to pickup | trip duration | fare to riders | fare to drivers)
    if !telemetry.completed_trips.is_empty() {
        let n = telemetry.completed_trips.len();

        let match_dist = timing_distribution(telemetry.completed_trips.as_slice(), CompletedTripRecord::time_to_match);
        let pickup_dist = timing_distribution(telemetry.completed_trips.as_slice(), CompletedTripRecord::time_to_pickup);
        let trip_dist = timing_distribution(telemetry.completed_trips.as_slice(), CompletedTripRecord::trip_duration);

        let rider_fares: Vec<f64> = telemetry.completed_trips.iter().map(|t| t.fare).collect();
        let driver_takes: Vec<f64> = telemetry
            .completed_trips
            .iter()
            .map(|t| t.fare * (1.0 - app.commission_rate))
            .collect();
        let (rider_min, rider_max, rider_avg, rider_sorted) =
            fare_distribution_stats(&rider_fares);
        let (driver_min, driver_max, driver_avg, driver_sorted) =
            fare_distribution_stats(&driver_takes);

        ui.columns(5, |columns| {
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
            columns[3].vertical(|ui| {
                ui.label(format!("Fare to riders (n={})", n));
                ui.label(format!("min {:.2}  max {:.2}", rider_min, rider_max));
                for (p, label) in PERCENTILES {
                    if let Some(v) = percentile_f64_sorted(&rider_sorted, *p) {
                        ui.label(format!("{} {:.2}", label, v));
                    }
                }
                ui.label(format!("avg {:.2}", rider_avg));
            });
            columns[4].vertical(|ui| {
                ui.label(format!("Fare to drivers (n={})", n));
                ui.label(format!("min {:.2}  max {:.2}", driver_min, driver_max));
                for (p, label) in PERCENTILES {
                    if let Some(v) = percentile_f64_sorted(&driver_sorted, *p) {
                        ui.label(format!("{} {:.2}", label, v));
                    }
                }
                ui.label(format!("avg {:.2}", driver_avg));
            });
        });
    }
}

/// Render fleet metrics: utilization, state breakdown, daily targets, fatigue.
fn render_fleet(ui: &mut egui::Ui, app: &SimUiApp) {
    let snapshots = match app.world.get_resource::<SimSnapshots>() {
        Some(s) => s,
        None => {
            ui.label("—");
            return;
        }
    };
    let clock = match app.world.get_resource::<sim_core::clock::SimulationClock>() {
        Some(c) => c,
        None => {
            ui.label("—");
            return;
        }
    };
    let latest = match snapshots.snapshots.back() {
        Some(s) => s,
        None => {
            ui.label("—");
            return;
        }
    };

    let c = &latest.counts;
    let sim_now_ms = clock.now();
    let total_drivers = c.drivers_idle
        + c.drivers_evaluating
        + c.drivers_en_route
        + c.drivers_on_trip
        + c.drivers_off_duty;
    let active_drivers = total_drivers.saturating_sub(c.drivers_off_duty);
    let busy = c.drivers_en_route + c.drivers_on_trip;
    let utilization_pct = if active_drivers > 0 {
        (busy as f64 / active_drivers as f64) * 100.0
    } else {
        0.0
    };

    let (sum_earnings, sum_target, targets_met) = latest.drivers.iter().fold(
        (0.0_f64, 0.0_f64, 0_usize),
        |(sum_e, sum_t, met), d| {
            let e = d.daily_earnings.unwrap_or(0.0);
            let t = d.daily_earnings_target.unwrap_or(0.0);
            let met_inc = if t > 0.0 && e >= t { 1 } else { 0 };
            (sum_e + e, sum_t + t, met + met_inc)
        },
    );
    let drivers_with_earnings = latest
        .drivers
        .iter()
        .filter(|d| d.daily_earnings.is_some())
        .count();
    let avg_earnings = if drivers_with_earnings > 0 {
        sum_earnings / drivers_with_earnings as f64
    } else {
        0.0
    };
    let avg_target = if drivers_with_earnings > 0 {
        sum_target / drivers_with_earnings as f64
    } else {
        0.0
    };

    let mut earnings_values: Vec<f64> = latest
        .drivers
        .iter()
        .filter_map(|d| d.daily_earnings)
        .collect();
    earnings_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n_earnings = earnings_values.len();
    let (earnings_min, earnings_mean, earnings_max) = if n_earnings == 0 {
        (0.0, 0.0, 0.0)
    } else {
        let min = earnings_values.first().copied().unwrap_or(0.0);
        let max = earnings_values.last().copied().unwrap_or(0.0);
        let mean = earnings_values.iter().sum::<f64>() / n_earnings as f64;
        (min, mean, max)
    };

    let mut ratio_values: Vec<f64> = latest
        .drivers
        .iter()
        .filter_map(|d| {
            let e = d.daily_earnings?;
            let t = d.daily_earnings_target?;
            if t > 0.0 {
                Some(e / t)
            } else {
                None
            }
        })
        .collect();
    ratio_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n_ratio = ratio_values.len();
    let (ratio_min, ratio_mean, ratio_max) = if n_ratio == 0 {
        (0.0, 0.0, 0.0)
    } else {
        let min = ratio_values.first().copied().unwrap_or(0.0);
        let max = ratio_values.last().copied().unwrap_or(0.0);
        let mean = ratio_values.iter().sum::<f64>() / n_ratio as f64;
        (min, mean, max)
    };

    fn percentile_f64(sorted: &[f64], p: u8) -> Option<f64> {
        if sorted.is_empty() || p > 100 {
            return None;
        }
        let n = sorted.len();
        let idx = ((p as usize) * (n - 1)) / 100;
        Some(sorted[idx])
    }

    let mut at_fatigue_limit = 0_usize;
    let mut session_durations: Vec<u64> = Vec::new();
    let mut fatigue_thresholds: Vec<u64> = Vec::new();
    for d in &latest.drivers {
        if let (Some(start), Some(threshold)) = (d.session_start_time_ms, d.fatigue_threshold_ms) {
            let mut session_ms = sim_now_ms.saturating_sub(start);
            // OffDuty drivers went off at or before threshold; cap so we don't count time after they went off
            if d.state == DriverState::OffDuty {
                session_ms = session_ms.min(threshold);
            }
            if session_ms >= threshold {
                at_fatigue_limit += 1;
            }
            session_durations.push(session_ms);
            fatigue_thresholds.push(threshold);
        }
    }
    let n_fatigue = session_durations.len();
    let (session_min, session_mean, session_max) = if n_fatigue == 0 {
        (0u64, 0u64, 0u64)
    } else {
        let min = *session_durations.iter().min().unwrap();
        let max = *session_durations.iter().max().unwrap();
        let mean = session_durations.iter().sum::<u64>() / n_fatigue as u64;
        (min, mean, max)
    };
    let (fatigue_min, fatigue_mean, fatigue_max) = if fatigue_thresholds.is_empty() {
        (0u64, 0u64, 0u64)
    } else {
        let min = *fatigue_thresholds.iter().min().unwrap();
        let max = *fatigue_thresholds.iter().max().unwrap();
        let mean = fatigue_thresholds.iter().sum::<u64>() / fatigue_thresholds.len() as u64;
        (min, mean, max)
    };

    ui.columns(5, |columns| {
        columns[0].vertical(|ui| {
            ui.label("Utilization (busy %)");
            ui.label(format!("{:.1}%", utilization_pct));
            ui.label("Total drivers");
            ui.label(total_drivers.to_string());
            ui.label("Active (not off duty)");
            ui.label(active_drivers.to_string());
        });
        columns[1].vertical(|ui| {
            ui.label("State breakdown:");
            ui.label(format!(
                "idle {} eval {} en_route {} on_trip {} off_duty {}",
                c.drivers_idle,
                c.drivers_evaluating,
                c.drivers_en_route,
                c.drivers_on_trip,
                c.drivers_off_duty
            ));
            if total_drivers > 0 {
                ui.label(format!(
                    "Idle {:.0}% | En route {:.0}% | On trip {:.0}% | Off duty {:.0}%",
                    (c.drivers_idle as f64 / total_drivers as f64) * 100.0,
                    (c.drivers_en_route as f64 / total_drivers as f64) * 100.0,
                    (c.drivers_on_trip as f64 / total_drivers as f64) * 100.0,
                    (c.drivers_off_duty as f64 / total_drivers as f64) * 100.0
                ));
            }
        });
        columns[2].vertical(|ui| {
            ui.label("Sum daily earnings");
            ui.label(format!("{:.1}", sum_earnings));
            ui.label("Sum daily targets");
            ui.label(format!("{:.1}", sum_target));
            ui.label("Targets met");
            ui.label(targets_met.to_string());
            ui.label("Off duty");
            ui.label(c.drivers_off_duty.to_string());
            ui.label("Avg earnings / driver");
            ui.label(format!("{:.1}", avg_earnings));
            ui.label("Avg target / driver");
            ui.label(format!("{:.1}", avg_target));
        });
        columns[3].vertical(|ui| {
            if n_earnings > 0 {
                ui.label(format!("Earnings distribution (n={})", n_earnings));
                ui.label(format!(
                    "min {:.1}  max {:.1}  avg {:.1}",
                    earnings_min, earnings_max, earnings_mean
                ));
                for (p, label) in [(50, "p50"), (90, "p90"), (95, "p95"), (99, "p99")] {
                    if let Some(v) = percentile_f64(&earnings_values, p) {
                        ui.label(format!("{} {:.1}", label, v));
                    }
                }
            }
            if n_ratio > 0 {
                ui.add_space(4.0);
                ui.label(format!("Earnings/target distribution (n={})", n_ratio));
                ui.label(format!(
                    "min {:.2}  max {:.2}  avg {:.2}",
                    ratio_min, ratio_max, ratio_mean
                ));
                for (p, label) in [(50, "p50"), (90, "p90"), (95, "p95"), (99, "p99")] {
                    if let Some(v) = percentile_f64(&ratio_values, p) {
                        ui.label(format!("{} {:.2}", label, v));
                    }
                }
            }
        });
        columns[4].vertical(|ui| {
            ui.label("At fatigue limit");
            ui.label(at_fatigue_limit.to_string());
            ui.label("Session duration (min / avg / max)");
            ui.label(format!(
                "{} / {} / {}",
                format_hms_from_ms(session_min),
                format_hms_from_ms(session_mean),
                format_hms_from_ms(session_max)
            ));
            ui.label("Fatigue threshold (min / avg / max)");
            ui.label(format!(
                "{} / {} / {}",
                format_hms_from_ms(fatigue_min),
                format_hms_from_ms(fatigue_mean),
                format_hms_from_ms(fatigue_max)
            ));
            if n_fatigue > 0 {
                ui.label(format!("Drivers with fatigue data: {}", n_fatigue));
            }
        });
    });
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

/// Min, max, mean and sorted copy for fare distribution (percentiles).
fn fare_distribution_stats(values: &[f64]) -> (f64, f64, f64, Vec<f64>) {
    if values.is_empty() {
        return (0.0, 0.0, 0.0, Vec::new());
    }
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mean = values.iter().sum::<f64>() / (values.len() as f64);
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    (min, max, mean, sorted)
}

/// Nearest-rank percentile (0–100) for sorted f64 slice.
fn percentile_f64_sorted(sorted: &[f64], p: u8) -> Option<f64> {
    if sorted.is_empty() || p > 100 {
        return None;
    }
    let n = sorted.len();
    let idx = ((p as usize) * (n - 1)) / 100;
    Some(sorted[idx])
}

/// Render scenario parameter inputs in a seven-column layout organized by category.
fn render_scenario_parameters(ui: &mut egui::Ui, app: &mut SimUiApp) {
    let can_edit = !app.started;

    ui.columns(7, |columns| {
        // Col 1: Supply (drivers - initial, spawn count, spread)
        columns[0].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("Supply (Drivers)");
            });
            ui.horizontal(|ui| {
                ui.label("Initial").on_hover_text("Number of drivers spawned immediately at simulation start (before scheduled spawning)");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.initial_driver_count).range(0..=10_000),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Spawn count").on_hover_text("Total number of drivers to spawn over the simulation window");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.num_drivers).range(1..=10_000),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Spread (h)").on_hover_text("Time window (hours) over which scheduled drivers spawn. Drivers spawn continuously with time-of-day variations (rush hours have higher rates)");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.driver_spread_hours).range(1..=24),
                );
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Driver Decision");
            });
            ui.horizontal(|ui| {
                ui.label("Base score").on_hover_text("Base acceptance score before factors are applied. Higher values make drivers more likely to accept matches. Used in logit model: score = base + (fare × fare_weight) + (pickup_distance × pickup_penalty) + ...");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.driver_base_acceptance_score)
                        .range(-10.0..=10.0)
                        .speed(0.1),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Fare weight").on_hover_text("Weight for fare attractiveness in driver acceptance. Higher fare increases acceptance probability. Typical range: 0.0-1.0");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.driver_fare_weight)
                        .range(0.0..=1.0)
                        .speed(0.01),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Pickup penalty").on_hover_text("Penalty per km of pickup distance. Longer pickup distances decrease acceptance probability. Typically negative (e.g., -2.0 means each km reduces score by 2.0)");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.driver_pickup_distance_penalty)
                        .range(-10.0..=0.0)
                        .speed(0.1),
                );
            });
        });

        // Col 2: Demand (riders - initial, spawn count, spread, cancel)
        columns[1].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("Demand (Riders)");
            });
            ui.horizontal(|ui| {
                ui.label("Initial").on_hover_text("Number of riders spawned immediately at simulation start (before scheduled spawning)");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.initial_rider_count).range(0..=10_000),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Spawn count").on_hover_text("Total number of riders to spawn over the simulation window");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.num_riders).range(1..=10_000),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Spread (h)").on_hover_text("Time window (hours) over which scheduled riders spawn. Riders spawn with time-of-day variations (rush hours have higher demand rates)");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.request_window_hours).range(1..=24),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Cancel wait (m)").on_hover_text("Random wait time range (minutes) before rider cancels while waiting for pickup. Uniform distribution between min and max");
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
        });

        // Col 3: Pricing (base, per km, commission, surge)
        columns[2].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("Pricing");
            });
            ui.horizontal(|ui| {
                ui.label("Base fare").on_hover_text("Base fare in currency units (e.g., dollars). Formula: fare = base_fare + (distance_km × per_km_rate)");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.base_fare)
                        .range(0.0..=100.0)
                        .speed(0.1),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Per km").on_hover_text("Per-kilometer rate in currency units. Multiplied by trip distance and added to base fare");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.per_km_rate)
                        .range(0.0..=100.0)
                        .speed(0.1),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Commission").on_hover_text("Commission rate as fraction (0.0-1.0). 0.15 = 15% commission. Driver earnings = fare × (1 - commission_rate), platform revenue = fare × commission_rate");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.commission_rate)
                        .range(0.0..=1.0)
                        .speed(0.01)
                        .custom_formatter(|n, _| format!("{:.1}%", n * 100.0)),
                );
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_enabled(can_edit, egui::Checkbox::new(&mut app.surge_enabled, "Surge pricing")).on_hover_text("When enabled, applies dynamic surge multipliers when demand exceeds supply in local H3 clusters around pickup location");
            });
            ui.horizontal(|ui| {
                ui.label("Surge radius (k)").on_hover_text("H3 grid disk radius (k) for surge cluster calculation around pickup. Larger radius considers more drivers/riders in the area");
                ui.add_enabled(
                    can_edit && app.surge_enabled,
                    egui::DragValue::new(&mut app.surge_radius_k).range(1..=5).speed(1),
                );
                ui.label("Max mult").on_hover_text("Maximum surge multiplier cap (e.g., 2.0 = 2x base fare). Surge = min(1.0 + (demand - supply) / supply, max_multiplier)");
                ui.add_enabled(
                    can_edit && app.surge_enabled,
                    egui::DragValue::new(&mut app.surge_max_multiplier)
                        .range(1.0..=5.0)
                        .speed(0.1),
                );
            });
        });

        // Col 4: Rider quote
        columns[3].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("Rider quote");
            });
            ui.horizontal(|ui| {
                ui.label("Max WTP ($)").on_hover_text("Maximum willingness to pay. Rider will reject quote if fare exceeds this amount (deterministic rejection)");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.max_willingness_to_pay)
                        .range(1.0..=500.0)
                        .speed(1.0),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Max ETA (min)").on_hover_text("Maximum acceptable ETA to pickup (minutes). Rider will reject quote if ETA exceeds this (deterministic rejection)");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.max_acceptable_eta_min)
                        .range(1..=60)
                        .speed(1),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Accept %").on_hover_text("Probability (0.0-1.0) that rider accepts quote when within price/ETA limits. If rejected, rider may request another quote (up to max rejections)");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.accept_probability)
                        .range(0.0..=1.0)
                        .speed(0.05),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Max quote rejections").on_hover_text("Maximum number of quote rejections before rider gives up. After this, rider is marked as abandoned-quote and despawned");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.max_quote_rejections).range(1..=10).speed(1),
                );
            });
        });

        // Col 5: Matching
        columns[4].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("Matching");
            });
            ui.horizontal(|ui| {
                let old_algorithm = app.matching_algorithm;
                let combo_response = egui::ComboBox::from_id_salt("matching_algorithm")
                    .selected_text(match app.matching_algorithm {
                        MatchingAlgorithmType::Simple => "Simple",
                        MatchingAlgorithmType::CostBased => "Cost-based",
                        MatchingAlgorithmType::Hungarian => "Hungarian (batch)",
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
                        ui.selectable_value(
                            &mut app.matching_algorithm,
                            MatchingAlgorithmType::Hungarian,
                            "Hungarian (batch)",
                        );
                    });
                combo_response.response.on_hover_text("Matching algorithm: Simple = first match within radius; Cost-based = best match by distance+ETA score; Hungarian = global batch optimization (requires batch matching enabled)");
                if app.matching_algorithm != old_algorithm {
                    app.matching_algorithm_changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.add_enabled(can_edit, egui::Checkbox::new(&mut app.batch_matching_enabled, "Batch matching")).on_hover_text("When enabled, collects all waiting riders and idle drivers periodically and runs global matching. When disabled, each rider triggers TryMatch individually");
            });
            ui.horizontal(|ui| {
                ui.label("Batch interval (s)").on_hover_text("Interval (seconds) between batch matching runs. Only applies when batch matching is enabled");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.batch_interval_secs)
                        .range(1..=120)
                        .speed(1),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Match radius (km)").on_hover_text("Maximum H3 grid distance for matching rider to driver. Converted to H3 cells internally. 0 = same cell only. Larger radius allows matching to drivers further away");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.match_radius_km)
                        .range(0.0..=20.0)
                        .speed(0.1),
                );
            });
        });

        // Col 6: Map and trips
        columns[5].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("Map & Trips");
            });
            ui.horizontal(|ui| {
                ui.label("Map size (km)").on_hover_text("Geographic bounds for spawn positions (lat/lng degrees). Default: San Francisco Bay Area. Larger size = larger playable area");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.map_size_km)
                        .range(1.0..=200.0)
                        .speed(1.0),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Trip (km)").on_hover_text("Trip length range (km). Converted to H3 cells internally. Random destination is selected within this range from pickup location");
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

        // Col 7: Timing
        columns[6].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("Timing");
            });
            ui.horizontal(|ui| {
                ui.label("Start (UTC)").on_hover_text("Real-world datetime (UTC) corresponding to simulation time 0. Affects time-of-day patterns (rush hours, day/night variations)");
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
                ui.label("Sim duration (h)").on_hover_text("Simulation stops when clock reaches this time (hours of sim time). Runner stops processing events once next event would be at or after this time");
                ui.add_enabled(
                    can_edit,
                    egui::DragValue::new(&mut app.simulation_duration_hours)
                        .range(1..=168)
                        .speed(1),
                );
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_enabled(can_edit, egui::Checkbox::new(&mut app.seed_enabled, "Seed")).on_hover_text("When enabled, uses the seed value for reproducible random number generation. Same seed = same simulation results");
                ui.add_enabled(
                    can_edit && app.seed_enabled,
                    egui::DragValue::new(&mut app.seed_value).range(0..=u64::MAX),
                );
            });
        });
    });
}
