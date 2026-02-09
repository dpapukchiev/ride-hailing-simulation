//! Control panel UI for simulation parameters and actions.

mod outcomes;
mod scenario;
mod topbar;

use eframe::egui;

use crate::app::SimUiApp;
use crate::ui::controls::outcomes::{render_fleet, render_run_outcomes};
use crate::ui::controls::scenario::render_scenario_parameters;
use crate::ui::controls::topbar::render_top_controls;

pub fn render_control_panel(ui: &mut egui::Ui, app: &mut SimUiApp) {
    render_top_controls(ui, app);

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
