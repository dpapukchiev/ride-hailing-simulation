use eframe::egui;

use sim_core::telemetry::{CompletedTripRecord, SimSnapshots, SimTelemetry};

use crate::app::SimUiApp;
use crate::ui::utils::format_hms_from_ms;

/// Render run outcome counters and optional completed-trip timing stats from SimTelemetry.
pub(super) fn render_run_outcomes(ui: &mut egui::Ui, app: &SimUiApp) {
    const PERCENTILES: &[(u8, &str)] = &[(50, "p50"), (90, "p90"), (95, "p95"), (99, "p99")];

    let telemetry = match app.world.get_resource::<SimTelemetry>() {
        Some(t) => t,
        None => {
            ui.label("—");
            return;
        }
    };

    // Outcome counters: 5 columns
    let total_resolved = telemetry
        .riders_completed_total
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
                let timeout_pct = (telemetry.riders_cancelled_pickup_timeout as f64
                    / telemetry.riders_cancelled_total as f64)
                    * 100.0;
                ui.add_space(2.0);
                ui.label(format!(
                    "  Timeout: {} ({:.1}%)",
                    telemetry.riders_cancelled_pickup_timeout, timeout_pct
                ));
            }
        });
        columns[2].vertical(|ui| {
            ui.label("Abandoned (quote)");
            ui.label(telemetry.riders_abandoned_quote_total.to_string());
            let total_quote_abandoned = telemetry.riders_abandoned_quote_total;
            if total_quote_abandoned > 0 {
                let price_pct = (telemetry.riders_abandoned_price as f64
                    / total_quote_abandoned as f64)
                    * 100.0;
                let eta_pct =
                    (telemetry.riders_abandoned_eta as f64 / total_quote_abandoned as f64) * 100.0;
                let stochastic_pct = (telemetry.riders_abandoned_stochastic as f64
                    / total_quote_abandoned as f64)
                    * 100.0;
                ui.add_space(2.0);
                ui.label(format!(
                    "  Price: {} ({:.1}%)",
                    telemetry.riders_abandoned_price, price_pct
                ));
                ui.label(format!(
                    "  ETA: {} ({:.1}%)",
                    telemetry.riders_abandoned_eta, eta_pct
                ));
                ui.label(format!(
                    "  Stochastic: {} ({:.1}%)",
                    telemetry.riders_abandoned_stochastic, stochastic_pct
                ));
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
                    ui.label(format!(
                        "browsing {} waiting {} in transit {}",
                        c.riders_browsing, c.riders_waiting, c.riders_in_transit
                    ));
                });
                columns[1].vertical(|ui| {
                    ui.label("Drivers now:");
                    ui.label(format!(
                        "idle {} en route {} on trip {} off duty {}",
                        c.drivers_idle, c.drivers_en_route, c.drivers_on_trip, c.drivers_off_duty
                    ));
                });
                columns[2].vertical(|ui| {
                    ui.label("Trips now:");
                    ui.label(format!(
                        "en route {} on trip {}",
                        c.trips_en_route, c.trips_on_trip
                    ));
                });
            });
        }
    }

    ui.add_space(8.0);

    // Timing and fare distributions: 6 columns (time to match | time to pickup | trip duration | fare to riders | fare to drivers | surge impact)
    if !telemetry.completed_trips.is_empty() {
        let n = telemetry.completed_trips.len();

        let match_dist = timing_distribution(
            telemetry.completed_trips.as_slice(),
            CompletedTripRecord::time_to_match,
        );
        let pickup_dist = timing_distribution(
            telemetry.completed_trips.as_slice(),
            CompletedTripRecord::time_to_pickup,
        );
        let trip_dist = timing_distribution(
            telemetry.completed_trips.as_slice(),
            CompletedTripRecord::trip_duration,
        );

        let rider_fares: Vec<f64> = telemetry.completed_trips.iter().map(|t| t.fare).collect();
        let driver_takes: Vec<f64> = telemetry
            .completed_trips
            .iter()
            .map(|t| t.fare * (1.0 - app.commission_rate))
            .collect();
        // Only include trips where surge actually increased the price
        let surge_impacts: Vec<f64> = telemetry
            .completed_trips
            .iter()
            .filter_map(|t| {
                if t.surge_impact > 0.0 {
                    Some(t.surge_impact)
                } else {
                    None
                }
            })
            .collect();
        let (rider_min, rider_max, rider_avg, rider_sorted) = fare_distribution_stats(&rider_fares);
        let (driver_min, driver_max, driver_avg, driver_sorted) =
            fare_distribution_stats(&driver_takes);
        let (surge_min, surge_max, surge_avg, surge_sorted) =
            fare_distribution_stats(&surge_impacts);
        let n_surge = surge_impacts.len();

        ui.columns(6, |columns| {
            columns[0].vertical(|ui| {
                ui.label(format!("Time to match (n={})", n));
                ui.label(format!(
                    "min {}  max {}",
                    format_hms_from_ms(match_dist.min),
                    format_hms_from_ms(match_dist.max)
                ));
                for (p, label) in PERCENTILES {
                    if let Some(v) = match_dist.percentile(*p) {
                        ui.label(format!("{} {}", label, format_hms_from_ms(v)));
                    }
                }
                ui.label(format!("avg {}", format_hms_from_ms(match_dist.mean)));
            });
            columns[1].vertical(|ui| {
                ui.label(format!("Time to pickup (n={})", n));
                ui.label(format!(
                    "min {}  max {}",
                    format_hms_from_ms(pickup_dist.min),
                    format_hms_from_ms(pickup_dist.max)
                ));
                for (p, label) in PERCENTILES {
                    if let Some(v) = pickup_dist.percentile(*p) {
                        ui.label(format!("{} {}", label, format_hms_from_ms(v)));
                    }
                }
                ui.label(format!("avg {}", format_hms_from_ms(pickup_dist.mean)));
            });
            columns[2].vertical(|ui| {
                ui.label(format!("Trip duration (n={})", n));
                ui.label(format!(
                    "min {}  max {}",
                    format_hms_from_ms(trip_dist.min),
                    format_hms_from_ms(trip_dist.max)
                ));
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
            columns[5].vertical(|ui| {
                if n_surge > 0 {
                    ui.label(format!("Surge impact (n={})", n_surge));
                    ui.label(format!("min {:.2}  max {:.2}", surge_min, surge_max));
                    for (p, label) in PERCENTILES {
                        if let Some(v) = percentile_f64_sorted(&surge_sorted, *p) {
                            ui.label(format!("{} {:.2}", label, v));
                        }
                    }
                    ui.label(format!("avg {:.2}", surge_avg));
                } else {
                    ui.label("Surge impact (n=0)");
                    ui.label("No trips with surge pricing");
                }
            });
        });
    }
}

/// Render fleet metrics: utilization, state breakdown, daily targets, fatigue.
pub(super) fn render_fleet(ui: &mut egui::Ui, app: &SimUiApp) {
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

    let (sum_earnings, sum_target, targets_met) =
        latest
            .drivers
            .iter()
            .fold((0.0_f64, 0.0_f64, 0_usize), |(sum_e, sum_t, met), d| {
                let e = d.daily_earnings.unwrap_or(0.0);
                let t = d.daily_earnings_target.unwrap_or(0.0);
                let met_inc = if t > 0.0 && e >= t { 1 } else { 0 };
                (sum_e + e, sum_t + t, met + met_inc)
            });
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
            let end = d.session_end_time_ms.unwrap_or(sim_now_ms);
            let session_ms = end.saturating_sub(start);
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
fn timing_distribution(
    trips: &[CompletedTripRecord],
    f: fn(&CompletedTripRecord) -> u64,
) -> TimingDistribution {
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
