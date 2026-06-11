use bevy_egui::egui::{self, RichText, ScrollArea, Ui};
use crate::plugins::defmt_decode::DefmtState;
use crate::plugins::serial::{ReconnectState, SerialChannel, SerialCommand, PortScanner, COMMON_BAUDS};
use crate::state::{ConnectedDevices, Encoding, LineEnding, LinkStatus, LogKind, LogLine, SerialBuffer, SerialInput};
use super::{export, theme};

const COLOR_SYSTEM: egui::Color32 = theme::TEXT3;
const COLOR_INFO:   egui::Color32 = theme::TEXT2;
const COLOR_WARN:   egui::Color32 = theme::WARN;
const COLOR_ERROR:  egui::Color32 = theme::DANGER;

#[allow(clippy::too_many_arguments)]
pub fn draw(
    ui: &mut Ui,
    buf: &mut SerialBuffer,
    input: &mut SerialInput,
    ch: &SerialChannel,
    scanner: &mut PortScanner,
    devices: &ConnectedDevices,
    defmt: &DefmtState,
    reconnect: &mut ReconnectState,
) {
    let connected = devices.serial
        .as_ref()
        .map(|s| matches!(s.status, LinkStatus::Connected))
        .unwrap_or(false);

    egui::TopBottomPanel::bottom("serial_send_bar")
        .resizable(false)
        .show_inside(ui, |ui| send_bar(ui, buf, input, ch));

    egui::TopBottomPanel::top("serial_header")
        .resizable(false)
        .show_inside(ui, |ui| {
            connect_bar(ui, scanner, ch, connected, buf, reconnect);
            ui.separator();
            toolbar(ui, buf, input, defmt);
        });

    if scanner.new_port_hint.is_some() {
        egui::TopBottomPanel::top("new_port_banner")
            .resizable(false)
            .show_inside(ui, |ui| {
                new_port_banner(ui, scanner, ch, connected, buf);
            });
    }

    // Terminal well — darkest surface, like a CRT phosphor screen.
    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(theme::BG_DEEP)
                .inner_margin(egui::Margin::symmetric(10.0, 6.0)),
        )
        .show_inside(ui, |ui| {
        let lines: Vec<&LogLine> = buf.filtered().collect();
        let total = buf.lines.len();
        let shown = lines.len();

        let row_height = ui.text_style_height(&egui::TextStyle::Monospace);

        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(!buf.paused)
            .show_rows(ui, row_height, shown, |ui, range| {
                for line in &lines[range] {
                    let color = line_color(&line.kind);
                    let text = if buf.show_timestamps {
                        format!("{}{}", fmt_ts(line.timestamp_ms), line.text)
                    } else {
                        line.text.clone()
                    };
                    ui.label(RichText::new(text).monospace().size(12.0).color(color));
                }
                if !buf.paused && buf.filter.is_empty() {
                    ui.label(RichText::new("█").monospace().size(12.0).color(theme::ACCENT));
                }
            });

        if !buf.filter.is_empty() {
            egui::TopBottomPanel::bottom("filter_count")
                .resizable(false)
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(format!("showing {shown} / {total} lines"))
                                .size(10.0).color(theme::TEXT3),
                        );
                    });
                });
        }
    });
}

// ── Connect bar ───────────────────────────────────────────────────────────────

fn connect_bar(
    ui: &mut Ui,
    scanner: &mut PortScanner,
    ch: &SerialChannel,
    connected: bool,
    buf: &mut SerialBuffer,
    reconnect: &mut ReconnectState,
) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_enabled_ui(!connected, |ui| {
            egui::ComboBox::from_id_salt("port_select")
                .selected_text(if scanner.selected.is_empty() { "no ports".into() } else { scanner.selected.clone() })
                .width(110.0)
                .show_ui(ui, |ui| {
                    for port in scanner.ports.clone() {
                        ui.selectable_value(&mut scanner.selected, port.clone(), &port);
                    }
                });
        });

        // Baud stays enabled while connected — watch_baud_change retunes the
        // open port live, Arduino-style.
        egui::ComboBox::from_id_salt("baud_select")
            .selected_text(format!("{}", scanner.baud))
            .width(85.0)
            .show_ui(ui, |ui| {
                for &baud in COMMON_BAUDS {
                    ui.selectable_value(&mut scanner.baud, baud, format!("{baud}"));
                }
            });

        ui.add_enabled_ui(!connected, |ui| {
            if ui.small_button("↺").on_hover_text("Refresh port list").clicked() {
                scanner.selected.clear();
            }
        });

        ui.separator();

        if connected {
            if theme::danger_button(ui, "Disconnect").clicked() {
                let _ = ch.tx.send(SerialCommand::Disconnect);
            }
        } else {
            ui.add_enabled_ui(!scanner.selected.is_empty(), |ui| {
                if theme::accent_button(ui, "Connect").clicked() {
                    do_connect(scanner, ch, buf);
                }
            });
        }

        // Auto-reconnect toggle.
        let auto_color = if reconnect.enabled { theme::ACCENT } else { theme::TEXT3 };
        if ui
            .add(egui::Button::new(RichText::new("↻ auto").size(11.0).color(auto_color)))
            .on_hover_text("Reconnect automatically when the port reappears")
            .clicked()
        {
            reconnect.enabled = !reconnect.enabled;
            if !reconnect.enabled {
                reconnect.waiting = false;
            }
        }

        let (badge, color) = if connected {
            ("● connected".to_string(), theme::ACCENT)
        } else if reconnect.waiting {
            let port = reconnect.target.as_ref().map(|(p, _)| p.as_str()).unwrap_or("port");
            (format!("⟳ waiting for {port}…"), theme::WARN)
        } else {
            ("○ disconnected".to_string(), theme::TEXT3)
        };
        ui.label(RichText::new(badge).size(11.0).color(color));
    });
    ui.add_space(4.0);
}

// ── New-port banner ───────────────────────────────────────────────────────────

fn new_port_banner(
    ui: &mut Ui,
    scanner: &mut PortScanner,
    ch: &SerialChannel,
    connected: bool,
    buf: &mut SerialBuffer,
) {
    let port = match &scanner.new_port_hint {
        Some(p) => p.clone(),
        None => return,
    };
    ui.add_space(3.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(
            RichText::new(format!("● New port {port} detected"))
                .size(11.0).color(theme::ACCENT),
        );
        if !connected && ui.small_button("Connect").clicked() {
            scanner.selected = port;
            scanner.new_port_hint = None;
            do_connect(scanner, ch, buf);
            return;
        }
        if ui.small_button("✕").clicked() {
            scanner.new_port_hint = None;
        }
    });
    ui.add_space(3.0);
}

// ── Toolbar: stats · view toggles · filter · export ──────────────────────────

fn toolbar(ui: &mut Ui, buf: &mut SerialBuffer, input: &mut SerialInput, defmt: &DefmtState) {
    ui.horizontal(|ui| {
        // ── Left: stats + controls ────────────────────────────────────────────
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!("RX {}", fmt_bytes(buf.rx_bytes)))
                .size(11.0).monospace().color(theme::TEXT3),
        );
        ui.separator();
        ui.label(
            RichText::new(format!("TX {}", fmt_bytes(buf.tx_bytes)))
                .size(11.0).monospace().color(theme::TEXT3),
        );
        ui.add_space(4.0);

        let pause_lbl = if buf.paused { "▶ Resume" } else { "⏸ Pause" };
        if ui.small_button(pause_lbl).clicked() { buf.paused = !buf.paused; }
        if ui.small_button("🗑 Clear").clicked() { buf.clear(); }

        ui.separator();

        // Timestamp toggle
        let ts_color = if buf.show_timestamps { theme::ACCENT } else { theme::TEXT3 };
        if ui.add(egui::Button::new(RichText::new("⏱").color(ts_color)))
            .on_hover_text("Toggle timestamps").clicked()
        {
            buf.show_timestamps = !buf.show_timestamps;
        }

        // Hex view toggle
        let hex_color = if buf.hex_view { theme::INFO } else { theme::TEXT3 };
        if ui.add(egui::Button::new(RichText::new("🔣").color(hex_color)))
            .on_hover_text("Hex dump view").clicked()
        {
            buf.hex_view = !buf.hex_view;
        }

        // defmt status badge (only when defmt encoding selected)
        if matches!(input.encoding, Encoding::Defmt) {
            ui.separator();
            let (label, color) = if defmt.status.is_ready() {
                ("defmt ✓", theme::ACCENT)
            } else {
                ("defmt: load ELF in Flash tab", theme::WARN)
            };
            ui.label(RichText::new(label).size(11.0).color(color))
                .on_hover_text(defmt.status.label());
        }

        // ── Right: filter + export ────────────────────────────────────────────
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Export
            if ui.small_button("💾").on_hover_text("Export log (txt / csv / json)").clicked() {
                export::log_dialog("serial_log.txt", &buf.filtered().collect::<Vec<_>>());
            }

            // Filter clear
            if !buf.filter.is_empty() && ui.small_button("✕").on_hover_text("Clear filter").clicked() {
                buf.filter.clear();
            }
            ui.add(
                egui::TextEdit::singleline(&mut buf.filter)
                    .desired_width(140.0)
                    .hint_text("🔍 filter…")
                    .font(egui::TextStyle::Monospace),
            );
        });
    });
}

// ── Send bar ──────────────────────────────────────────────────────────────────

fn send_bar(ui: &mut Ui, buf: &mut SerialBuffer, input: &mut SerialInput, ch: &SerialChannel) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("›").size(14.0).monospace().color(theme::ACCENT));

        let resp = ui.add(
            egui::TextEdit::singleline(&mut input.text)
                .desired_width(f32::INFINITY)
                .hint_text("send bytes…  (↑/↓ history)")
                .font(egui::TextStyle::Monospace)
                .frame(false),
        );

        // ↑/↓ recall previous commands while the input has focus.
        if resp.has_focus() {
            let (up, down) = ui.input(|i| {
                (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown))
            });
            let recalled = if up {
                input.history_prev().map(str::to_owned)
            } else if down {
                input.history_next().map(str::to_owned)
            } else {
                None
            };
            if let Some(text) = recalled {
                input.text = text;
            }
        }

        egui::ComboBox::from_id_salt("encoding")
            .selected_text(input.encoding.label())
            .width(60.0)
            .show_ui(ui, |ui| {
                for enc in [Encoding::Ascii, Encoding::Hex, Encoding::Defmt] {
                    ui.selectable_value(&mut input.encoding, enc, enc.label());
                }
            });

        egui::ComboBox::from_id_salt("line_ending")
            .selected_text(input.line_ending.label())
            .width(60.0)
            .show_ui(ui, |ui| {
                for le in LineEnding::ALL {
                    ui.selectable_value(&mut input.line_ending, le, le.label());
                }
            });

        let send_clicked = theme::accent_button(ui, "Send").clicked();
        let enter_pressed = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if send_clicked || enter_pressed {
            send_bytes(input, buf, ch);
            resp.request_focus();
        }
    });
    ui.add_space(4.0);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn do_connect(scanner: &PortScanner, ch: &SerialChannel, buf: &mut SerialBuffer) {
    buf.push(LogLine {
        timestamp_ms: 0,
        text: format!("opening {} @ {} 8N1...", scanner.selected, scanner.baud),
        kind: LogKind::System,
    });
    let _ = ch.tx.send(SerialCommand::Connect {
        port: scanner.selected.clone(),
        baud: scanner.baud,
    });
}

fn send_bytes(input: &mut SerialInput, buf: &mut SerialBuffer, ch: &SerialChannel) {
    if input.text.is_empty() { return; }
    input.push_history(input.text.clone());
    let mut bytes = match input.encoding {
        Encoding::Ascii | Encoding::Defmt => input.text.as_bytes().to_vec(),
        Encoding::Hex => input.text
            .split_whitespace()
            .filter_map(|s| u8::from_str_radix(s, 16).ok())
            .collect(),
    };
    bytes.extend_from_slice(input.line_ending.bytes());
    buf.tx_bytes += bytes.len();
    let _ = ch.tx.send(SerialCommand::Send(bytes));
    input.text.clear();
}

fn line_color(kind: &LogKind) -> egui::Color32 {
    match kind {
        LogKind::System => COLOR_SYSTEM,
        LogKind::Info   => COLOR_INFO,
        LogKind::Warn   => COLOR_WARN,
        LogKind::Error  => COLOR_ERROR,
    }
}

fn fmt_ts(ms: u64) -> String {
    let mins   = ms / 60_000;
    let secs   = (ms % 60_000) / 1_000;
    let millis = ms % 1_000;
    format!("[{mins:02}:{secs:02}.{millis:03}] ")
}

fn fmt_bytes(n: usize) -> String {
    if n >= 1024 * 1024 { format!("{:.1} MB", n as f64 / (1024.0 * 1024.0)) }
    else if n >= 1024   { format!("{:.1} KB", n as f64 / 1024.0) }
    else                { format!("{n} B") }
}
