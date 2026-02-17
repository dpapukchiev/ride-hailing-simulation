use eframe::egui;

use crate::app::{MatchingAlgorithmType, RoutingMode, SimUiApp, SpawnMode, TrafficProfileMode};
use crate::ui::utils::{datetime_from_unix_ms, now_unix_ms};

/// Render scenario parameter inputs in a seven-column layout organized by category.
pub(super) fn render_scenario_parameters(ui: &mut egui::Ui, app: &mut SimUiApp) {
    let can_edit = !app.started;

    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label("Presets");
            if let Some(active) = app.active_preset_name.as_ref() {
                ui.label(format!("Active: {active}"));
            } else {
                ui.label("Active: (none)");
            }
        });

        ui.horizontal(|ui| {
            ui.label("Name");
            ui.add_enabled(
                can_edit,
                egui::TextEdit::singleline(&mut app.preset_name_input).desired_width(180.0),
            );

            let selected_text = app
                .selected_preset_name
                .as_deref()
                .unwrap_or("Select preset")
                .to_string();
            egui::ComboBox::from_id_salt("preset_selector")
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    for preset_name in &app.preset_names {
                        ui.selectable_value(
                            &mut app.selected_preset_name,
                            Some(preset_name.clone()),
                            preset_name,
                        );
                    }
                });

            if ui
                .add_enabled(can_edit, egui::Button::new("Save preset"))
                .clicked()
            {
                app.save_named_preset();
            }
            if ui
                .add_enabled(
                    can_edit && app.selected_preset_name.is_some(),
                    egui::Button::new("Load preset"),
                )
                .clicked()
            {
                app.load_selected_preset();
            }
            if ui
                .add_enabled(
                    can_edit && app.selected_preset_name.is_some(),
                    egui::Button::new("Delete preset"),
                )
                .clicked()
            {
                app.delete_selected_preset();
            }
        });

        ui.horizontal(|ui| {
            ui.label("Transfer path").on_hover_text(
                "Path to a full-library preset transfer JSON file for export/import.",
            );
            ui.add_enabled(
                can_edit,
                egui::TextEdit::singleline(&mut app.preset_transfer_path_input)
                    .desired_width(260.0)
                    .hint_text("./preset-library-transfer.json"),
            );
            if ui
                .add_enabled(can_edit, egui::Button::new("Export library"))
                .clicked()
            {
                app.export_preset_library();
            }
            if ui
                .add_enabled(can_edit, egui::Button::new("Import library"))
                .clicked()
            {
                app.import_preset_library();
            }
        });
        ui.small("Export/import applies to the full preset library. Import replaces current library after validation succeeds.");

        if let Some(message) = app.preset_status_message.as_ref() {
            ui.colored_label(egui::Color32::from_rgb(220, 180, 80), message);
        }
    });
    ui.add_space(6.0);

    if let Some(overwrite_target) = app.pending_overwrite_name.clone() {
        egui::Window::new("Overwrite preset?")
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.label(format!(
                    "Preset '{overwrite_target}' already exists. Overwrite it?"
                ));
                ui.horizontal(|ui| {
                    if ui.button("Overwrite").clicked() {
                        app.confirm_overwrite_named_preset();
                    }
                    if ui.button("Cancel").clicked() {
                        app.cancel_overwrite_named_preset();
                    }
                });
            });
    }

    ui.columns(8, |columns| {
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
                ui.label("Surge radius (k)").on_hover_text("H3 grid disk radius (k) for surge cluster calculation around pickup. Larger radius considers more drivers/riders in the area, which may reduce how often surge pricing is applied by including more available drivers in the supply calculation");
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

        // Col 7: Routing & Traffic
        columns[6].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("Routing & Traffic");
            });
            ui.horizontal(|ui| {
                ui.label("Routing").on_hover_text(
                    "H3 Grid: fast, grid-based pathfinding (no external service needed).\n\
                     OSRM: real road-network routing via local OSRM server (requires Docker setup).",
                );
                let combo_response = egui::ComboBox::from_id_salt("routing_mode")
                    .selected_text(match app.routing_mode {
                        RoutingMode::H3Grid => "H3 Grid",
                        RoutingMode::Osrm => "OSRM",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut app.routing_mode,
                            RoutingMode::H3Grid,
                            "H3 Grid",
                        );
                        ui.selectable_value(
                            &mut app.routing_mode,
                            RoutingMode::Osrm,
                            "OSRM (Berlin)",
                        );
                    });
                combo_response.response.on_hover_text(
                    "Select routing backend for vehicle movement",
                );
            });
            if app.routing_mode == RoutingMode::Osrm {
                ui.horizontal(|ui| {
                    ui.label("Endpoint").on_hover_text(
                        "OSRM HTTP endpoint URL. Default: http://localhost:5000 \
                         (Docker setup in infra/osrm/)",
                    );
                    ui.add_enabled(
                        can_edit,
                        egui::TextEdit::singleline(&mut app.osrm_endpoint)
                            .desired_width(120.0),
                    );
                });
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Traffic").on_hover_text(
                    "Time-of-day speed profile.\n\
                     None: constant speed (no traffic effects).\n\
                     Berlin: realistic rush-hour slowdowns.",
                );
                egui::ComboBox::from_id_salt("traffic_profile")
                    .selected_text(match app.traffic_profile_mode {
                        TrafficProfileMode::None => "None",
                        TrafficProfileMode::Berlin => "Berlin",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut app.traffic_profile_mode,
                            TrafficProfileMode::None,
                            "None",
                        );
                        ui.selectable_value(
                            &mut app.traffic_profile_mode,
                            TrafficProfileMode::Berlin,
                            "Berlin",
                        );
                    });
            });
            ui.horizontal(|ui| {
                ui.add_enabled(
                    can_edit,
                    egui::Checkbox::new(&mut app.congestion_zones_enabled, "Congestion zones"),
                )
                .on_hover_text(
                    "Enable spatial congestion zones (city center and ring road). \
                     Slows vehicles in predefined high-traffic areas.",
                );
            });
            ui.horizontal(|ui| {
                ui.add_enabled(
                    can_edit,
                    egui::Checkbox::new(&mut app.dynamic_congestion_enabled, "Dynamic congestion"),
                )
                .on_hover_text(
                    "Enable density-based congestion: speed is reduced when \
                     many vehicles occupy the same H3 cell.",
                );
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Spawns").on_hover_text(
                    "Spawn location weighting for riders and drivers.\n\
                     Uniform: random positions within map bounds.\n\
                     Berlin Hotspots: weighted toward realistic city hotspots.",
                );
                egui::ComboBox::from_id_salt("spawn_mode")
                    .selected_text(match app.spawn_mode {
                        SpawnMode::Uniform => "Uniform",
                        SpawnMode::BerlinHotspots => "Berlin",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut app.spawn_mode,
                            SpawnMode::Uniform,
                            "Uniform",
                        );
                        ui.selectable_value(
                            &mut app.spawn_mode,
                            SpawnMode::BerlinHotspots,
                            "Berlin Hotspots",
                        );
                    });
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_enabled(
                    can_edit,
                    egui::Checkbox::new(&mut app.base_speed_enabled, "Base speed"),
                )
                .on_hover_text(
                    "Override the default free-flow speed (50 km/h). \
                     Traffic factors are multiplied against this base.",
                );
                ui.add_enabled(
                    can_edit && app.base_speed_enabled,
                    egui::DragValue::new(&mut app.base_speed_kmh)
                        .range(10.0..=200.0)
                        .speed(1.0)
                        .suffix(" km/h"),
                );
            });
        });

        // Col 8: Timing
        columns[7].vertical(|ui| {
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
