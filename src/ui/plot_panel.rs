use bevy_egui::egui::{self, RichText, Ui};
use egui_plot::{Legend, Line, Plot, PlotPoints};
use crate::state::PlotState;
use super::theme;

const SERIES_COLORS: [egui::Color32; 6] = [
    theme::ACCENT,
    theme::INFO,
    theme::WARN,
    theme::DANGER,
    egui::Color32::from_rgb(0xb0, 0x7f, 0xff), // violet
    egui::Color32::from_rgb(0xff, 0x9e, 0x4a), // orange
];

pub fn draw(ui: &mut Ui, state: &mut PlotState) {
    // ── Toolbar ───────────────────────────────────────────────────────────────
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(4.0);

        let pause_lbl = if state.paused { "▶ Resume" } else { "⏸ Pause" };
        if ui.small_button(pause_lbl).clicked() {
            state.paused = !state.paused;
        }
        if ui.small_button("🗑 Clear").clicked() {
            state.clear();
        }

        ui.separator();

        ui.label(RichText::new("Window").size(11.0).color(theme::TEXT3));
        ui.add(
            egui::Slider::new(&mut state.window, 100..=4096)
                .logarithmic(true)
                .suffix(" samples"),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!(
                    "{} series · {} samples",
                    state.series.len(),
                    state.sample_idx
                ))
                .size(11.0)
                .monospace()
                .color(theme::TEXT3),
            );
        });
    });
    ui.add_space(4.0);
    ui.separator();

    // ── Empty hint ────────────────────────────────────────────────────────────
    if state.series.is_empty() {
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("No plot data yet").size(13.0).color(theme::TEXT2));
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "Lines on the serial port made up entirely of numbers are charted here.",
                )
                .size(11.0)
                .color(theme::TEXT3),
            );
            ui.label(
                RichText::new("e.g.   1.0 2.5 3.7   or   temp:23.4,hum:40")
                    .size(11.0)
                    .monospace()
                    .color(theme::TEXT3),
            );
        });
        return;
    }

    // ── Chart ─────────────────────────────────────────────────────────────────
    let min_x = state.sample_idx.saturating_sub(state.window as u64) as f64;
    Plot::new("serial_plot")
        .legend(Legend::default())
        .allow_scroll(false)
        .show(ui, |plot_ui| {
            for (i, series) in state.series.iter().enumerate() {
                let pts: Vec<[f64; 2]> = series
                    .points
                    .iter()
                    .copied()
                    .filter(|p| p[0] >= min_x)
                    .collect();
                plot_ui.line(
                    Line::new(PlotPoints::from(pts))
                        .name(&series.name)
                        .color(SERIES_COLORS[i % SERIES_COLORS.len()])
                        .width(1.5),
                );
            }
        });
}
