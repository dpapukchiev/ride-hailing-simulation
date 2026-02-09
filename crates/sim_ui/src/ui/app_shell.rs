use std::time::{Duration, Instant};

use eframe::egui;

use sim_core::matching::MatchingAlgorithmResource;

use crate::app::SimUiApp;
use crate::ui::controls::render_control_panel;
use crate::ui::dashboard::render_dashboard;

pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_maximized(true),
        ..Default::default()
    };
    eframe::run_native(
        "Ride-Hailing Simulation",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_pixels_per_point(0.8);
            Ok(Box::new(SimUiApp::new()))
        }),
    )
}

impl eframe::App for SimUiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
            render_dashboard(ui, self);
        });
    }
}
