use super::theme;
use crate::plugins::network::NetworkState;
use crate::plugins::serial::PortScanner;
use crate::state::{ActiveTab, ConnectedDevices, LinkStatus};
use bevy_egui::egui::{self, Color32, CursorIcon, Id, RichText, Stroke, Ui};

pub fn draw(
    ui: &mut Ui,
    devices: &ConnectedDevices,
    active_tab: &mut ActiveTab,
    port_scanner: &mut PortScanner,
    net_state: &mut NetworkState,
) {
    // ── Header ────────────────────────────────────────────────────────────────
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.label(
            RichText::new("Rusty")
                .size(16.0)
                .strong()
                .monospace()
                .color(theme::TEXT),
        );
        // Tight kerning between the two halves of the wordmark.
        ui.add_space(-8.0);
        ui.label(
            RichText::new("MCU")
                .size(16.0)
                .strong()
                .monospace()
                .color(theme::ACCENT),
        );
    });
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(10.0);

    // ── Interfaces ────────────────────────────────────────────────────────────
    section_label(ui, "INTERFACES");
    ui.add_space(6.0);

    if let Some(probe) = &devices.debug_probe {
        if device_card(
            ui,
            "probe_card",
            &probe.status,
            &probe.name,
            &probe.transport,
            *active_tab == ActiveTab::Flash,
        ) {
            *active_tab = ActiveTab::Flash;
        }
    }

    if let Some(serial) = &devices.serial {
        let subtitle = format!("{} · {}", serial.baud, serial.framing);
        if device_card(
            ui,
            "serial_card",
            &serial.status,
            &serial.port,
            &subtitle,
            *active_tab == ActiveTab::Serial,
        ) {
            *active_tab = ActiveTab::Serial;
            // Auto-fill the port in the connect bar if it's available.
            if port_scanner.ports.contains(&serial.port) {
                port_scanner.selected = serial.port.clone();
                port_scanner.baud = serial.baud;
            }
        }
    }

    if let Some(eth) = &devices.ethernet {
        if device_card(
            ui,
            "eth_card",
            &eth.status,
            &eth.ip,
            &eth.speed,
            *active_tab == ActiveTab::Network,
        ) {
            *active_tab = ActiveTab::Network;
            // Auto-fill the host field with the board's IP.
            net_state.host = eth.ip.clone();
        }
    }

    if let Some(usb) = &devices.usb_hs {
        if device_card(
            ui,
            "usb_card",
            &usb.status,
            &usb.class,
            &usb.note,
            *active_tab == ActiveTab::Usb,
        ) {
            *active_tab = ActiveTab::Usb;
        }
    }

    if devices.debug_probe.is_none()
        && devices.serial.is_none()
        && devices.ethernet.is_none()
        && devices.usb_hs.is_none()
    {
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.label(
                RichText::new("no devices detected")
                    .size(11.0)
                    .italics()
                    .color(theme::TEXT3),
            );
        });
    }

    // ── Chip info ─────────────────────────────────────────────────────────────
    if let Some(chip) = &devices.chip {
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("CHIP").size(10.0).color(theme::TEXT3));
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(&chip.part)
                            .size(11.0)
                            .monospace()
                            .color(theme::TEXT2),
                    );
                    for line in [
                        chip.core.as_str(),
                        &format!("{}K flash · {}K RAM", chip.flash_kb, chip.ram_kb),
                        &format!("fw {}", chip.fw_version),
                    ] {
                        ui.label(
                            RichText::new(line)
                                .size(11.0)
                                .monospace()
                                .color(theme::TEXT3),
                        );
                    }
                    ui.add_space(8.0);
                });
            });
        });
    }
}

fn section_label(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.label(RichText::new(text).size(10.0).color(theme::TEXT3));
    });
}

// ── Card widget ───────────────────────────────────────────────────────────────
//
// Returns true if the card was clicked this frame.

fn device_card(
    ui: &mut Ui,
    id: &str,
    status: &LinkStatus,
    name: &str,
    subtitle: &str,
    active: bool,
) -> bool {
    let (fill, base_border) = if active {
        (theme::BG3, Stroke::new(1.5_f32, theme::ACCENT_DIM))
    } else {
        (theme::BG2, Stroke::new(1.0_f32, theme::BORDER_LIGHT))
    };

    let inner = egui::Frame::none()
        .outer_margin(egui::Margin::symmetric(10.0, 0.0))
        .fill(fill)
        .stroke(base_border)
        .rounding(egui::Rounding::same(6.0))
        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
        .show(ui, |ui| {
            ui.set_min_width(168.0);
            ui.horizontal(|ui| {
                // Status dot with phosphor glow when up.
                let (rect, _) = ui.allocate_exact_size(egui::vec2(9.0, 14.0), egui::Sense::hover());
                if status.is_up() {
                    ui.painter().circle_filled(
                        rect.center(),
                        5.5,
                        Color32::from_rgba_unmultiplied(0x00, 0xe8, 0x7a, 40),
                    );
                    ui.painter()
                        .circle_filled(rect.center(), 3.0, theme::ACCENT);
                } else {
                    ui.painter().circle_filled(rect.center(), 3.0, theme::TEXT3);
                }
                ui.add_space(4.0);

                ui.vertical(|ui| {
                    let name_color = if status.is_up() {
                        theme::TEXT
                    } else {
                        theme::TEXT3
                    };
                    ui.label(
                        RichText::new(name)
                            .size(12.0)
                            .strong()
                            .monospace()
                            .color(name_color),
                    );
                    ui.label(RichText::new(subtitle).size(11.0).color(theme::TEXT3));
                });
            });
        });

    // Overlay interaction on the drawn rect.
    let resp = ui
        .interact(inner.response.rect, Id::new(id), egui::Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);

    // Hover highlight — overdraw a brighter border.
    if resp.hovered() && !active {
        ui.painter().rect_stroke(
            inner.response.rect,
            6.0,
            Stroke::new(1.0_f32, theme::BORDER),
        );
    }

    ui.add_space(4.0);

    resp.clicked()
}
