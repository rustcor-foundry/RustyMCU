use bevy_egui::egui::{self, Color32, RichText, ScrollArea, Ui};
use crate::plugins::network::{NetworkState, TcpChannel, TcpCommand};
use crate::state::LogKind;

const COLOR_SYSTEM: Color32 = Color32::from_gray(110);
const COLOR_INFO:   Color32 = Color32::from_gray(190);
const COLOR_WARN:   Color32 = Color32::from_rgb(200, 130, 50);
const COLOR_ERROR:  Color32 = Color32::from_rgb(200, 80, 80);

pub fn draw(ui: &mut Ui, state: &mut NetworkState, ch: &TcpChannel) {
    // ── Bottom send bar ───────────────────────────────────────────────────────
    egui::TopBottomPanel::bottom("net_send_bar")
        .resizable(false)
        .show_inside(ui, |ui| send_bar(ui, state, ch));

    // ── Top: connect header + toolbar ─────────────────────────────────────────
    egui::TopBottomPanel::top("net_header")
        .resizable(false)
        .show_inside(ui, |ui| {
            connect_bar(ui, state, ch);
            ui.separator();
            stats_toolbar(ui, state);
        });

    // ── Log terminal ──────────────────────────────────────────────────────────
    egui::CentralPanel::default().show_inside(ui, |ui| {
        let row_height = ui.text_style_height(&egui::TextStyle::Monospace);

        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(!state.paused)
            .show_rows(ui, row_height, state.log.len(), |ui, range| {
                for line in state.log.range(range) {
                    let color = line_color(&line.kind);
                    ui.label(RichText::new(&line.text).monospace().size(12.0).color(color));
                }
                if !state.paused && state.connected {
                    ui.label(RichText::new("█").monospace().size(12.0).color(COLOR_SYSTEM));
                }
            });
    });
}

fn connect_bar(ui: &mut Ui, state: &mut NetworkState, ch: &TcpChannel) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        // Host input
        ui.add_enabled_ui(!state.connected, |ui| {
            ui.label(RichText::new("Host").size(11.0).color(Color32::GRAY));
            ui.add(
                egui::TextEdit::singleline(&mut state.host)
                    .desired_width(140.0)
                    .font(egui::TextStyle::Monospace),
            );
            ui.label(RichText::new("Port").size(11.0).color(Color32::GRAY));
            ui.add(
                egui::TextEdit::singleline(&mut state.port)
                    .desired_width(52.0)
                    .font(egui::TextStyle::Monospace),
            );
        });

        ui.separator();

        if state.connected {
            if ui
                .button(RichText::new("Disconnect").color(COLOR_WARN))
                .clicked()
            {
                let _ = ch.tx.send(TcpCommand::Disconnect);
            }
        } else {
            let port_ok = state.port.parse::<u16>().is_ok();
            let can_connect = !state.host.is_empty() && port_ok;
            ui.add_enabled_ui(can_connect, |ui| {
                if ui.button("Connect").clicked() {
                    if let Ok(port) = state.port.parse::<u16>() {
                        let _ = ch.tx.send(TcpCommand::Connect {
                            host: state.host.clone(),
                            port,
                        });
                    }
                }
            });
            if !state.port.is_empty() && state.port.parse::<u16>().is_err() {
                ui.label(RichText::new("invalid port").size(10.0).color(COLOR_ERROR));
            }
        }

        let (badge, color) = if state.connected {
            ("● connected", Color32::from_rgb(99, 153, 34))
        } else {
            ("○ disconnected", Color32::GRAY)
        };
        ui.label(RichText::new(badge).size(11.0).color(color));
    });
    ui.add_space(4.0);
}

fn stats_toolbar(ui: &mut Ui, state: &mut NetworkState) {
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!("RX {}", fmt_bytes(state.rx_bytes)))
                .size(11.0).monospace().color(Color32::GRAY),
        );
        ui.separator();
        ui.label(
            RichText::new(format!("TX {}", fmt_bytes(state.tx_bytes)))
                .size(11.0).monospace().color(Color32::GRAY),
        );
        ui.add_space(8.0);
        let pause_label = if state.paused { "▶ Resume" } else { "⏸ Pause" };
        if ui.small_button(pause_label).clicked() {
            state.paused = !state.paused;
        }
        if ui.small_button("🗑 Clear").clicked() {
            state.clear();
        }
    });
}

fn send_bar(ui: &mut Ui, state: &mut NetworkState, ch: &TcpChannel) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("›").size(14.0).monospace().color(Color32::GRAY));

        let resp = ui.add(
            egui::TextEdit::singleline(&mut state.input)
                .desired_width(f32::INFINITY)
                .hint_text("send bytes…")
                .font(egui::TextStyle::Monospace)
                .frame(false),
        );

        let send_clicked = ui.add_enabled(state.connected, egui::Button::new("Send")).clicked();
        let enter_pressed = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

        if (send_clicked || enter_pressed) && !state.input.is_empty() {
            let mut bytes = state.input.as_bytes().to_vec();
            bytes.push(b'\n');
            state.tx_bytes += bytes.len();
            let _ = ch.tx.send(TcpCommand::Send(bytes));
            state.input.clear();
            resp.request_focus();
        }
    });
    ui.add_space(4.0);
}

fn line_color(kind: &LogKind) -> Color32 {
    match kind {
        LogKind::System => COLOR_SYSTEM,
        LogKind::Info   => COLOR_INFO,
        LogKind::Warn   => COLOR_WARN,
        LogKind::Error  => COLOR_ERROR,
    }
}

fn fmt_bytes(n: usize) -> String {
    if n >= 1024 * 1024 {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    } else if n >= 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{n} B")
    }
}
