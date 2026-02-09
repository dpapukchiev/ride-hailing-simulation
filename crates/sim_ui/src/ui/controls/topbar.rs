use eframe::egui;

use crate::app::SimUiApp;
use crate::ui::utils::{format_datetime_from_unix_ms, format_hms_from_ms, now_unix_ms};

pub(super) fn render_top_controls(ui: &mut egui::Ui, app: &mut SimUiApp) {
    ui.horizontal(|ui| {
        let can_start = !app.started;
        if ui
            .add_enabled(can_start, egui::Button::new("Start"))
            .clicked()
        {
            app.start_simulation();
        }
        if ui
            .button(if app.auto_run { "Pause" } else { "Run" })
            .clicked()
            && app.started
        {
            app.auto_run = !app.auto_run;
            if app.auto_run {
                app.last_frame_instant = Some(std::time::Instant::now());
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
        ui.checkbox(
            &mut app.show_driver_stats,
            "Driver stats (earnings/fatigue)",
        );
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
        ui.label(format!("Sim time: {}", format_hms_from_ms(sim_now_ms)));
        ui.label(format!(
            "Sim datetime (UTC): {}",
            format_datetime_from_unix_ms(sim_real_ms)
        ));
        ui.label(format!(
            "Wall clock (UTC): {}",
            format_datetime_from_unix_ms(now_unix_ms())
        ));
    });
}
