use bevy_egui::egui::{self, Color32, CursorIcon, Id, RichText, Stroke, Ui};
use crate::plugins::network::NetworkState;
use crate::plugins::serial::PortScanner;
use crate::state::{ActiveTab, ConnectedDevices, LinkStatus};

const GREEN: Color32 = Color32::from_rgb(99, 153, 34);
const GRAY:  Color32 = Color32::from_rgb(136, 135, 128);

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
        ui.vertical(|ui| {
            ui.label(RichText::new("CH32V307").size(10.0).color(Color32::GRAY));
            ui.label(RichText::new("board-tools").size(15.0).strong().monospace());
        });
    });
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(10.0);

    // ── Interfaces ────────────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.label(RichText::new("INTERFACES").size(10.0).color(Color32::GRAY));
    });
    ui.add_space(6.0);

    if let Some(probe) = &devices.debug_probe {
        if device_card(
            ui, "probe_card",
            &probe.status, &probe.name, &probe.transport,
            *active_tab == ActiveTab::Flash,
        ) {
            *active_tab = ActiveTab::Flash;
        }
    }

    if let Some(serial) = &devices.serial {
        let subtitle = format!("{} · {}", serial.baud, serial.framing);
        if device_card(
            ui, "serial_card",
            &serial.status, &serial.port, &subtitle,
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
            ui, "eth_card",
            &eth.status, &eth.ip, &eth.speed,
            *active_tab == ActiveTab::Network,
        ) {
            *active_tab = ActiveTab::Network;
            // Auto-fill the host field with the board's IP.
            net_state.host = eth.ip.clone();
        }
    }

    if let Some(usb) = &devices.usb_hs {
        if device_card(
            ui, "usb_card",
            &usb.status, &usb.class, &usb.note,
            *active_tab == ActiveTab::Usb,
        ) {
            *active_tab = ActiveTab::Usb;
        }
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
                    ui.label(RichText::new("CHIP").size(10.0).color(Color32::GRAY));
                    ui.add_space(4.0);
                    for line in [
                        chip.part.as_str(),
                        chip.core.as_str(),
                        &format!("{}K flash · {}K RAM", chip.flash_kb, chip.ram_kb),
                        &format!("fw {}", chip.fw_version),
                    ] {
                        ui.label(RichText::new(line).size(11.0).monospace().color(Color32::GRAY));
                    }
                    ui.add_space(8.0);
                });
            });
        });
    }
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
    let dot_color = if status.is_up() { GREEN } else { GRAY };

    // Base border: thick + brighter when this card's tab is active.
    let base_border = if active {
        Stroke::new(1.5, Color32::from_gray(110))
    } else {
        Stroke::new(0.5, Color32::from_gray(55))
    };

    let inner = egui::Frame::none()
        .outer_margin(egui::Margin::symmetric(10.0, 0.0))
        .stroke(base_border)
        .rounding(egui::Rounding::same(4.0))
        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
        .show(ui, |ui| {
            ui.set_min_width(160.0);
            ui.horizontal(|ui| {
                // Status dot
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(7.0, 14.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 3.5, dot_color);
                ui.add_space(4.0);

                ui.vertical(|ui| {
                    let name_color = if status.is_up() {
                        Color32::from_gray(220)
                    } else {
                        Color32::from_gray(140)
                    };
                    ui.label(RichText::new(name).size(12.0).strong().monospace().color(name_color));
                    ui.label(RichText::new(subtitle).size(11.0).color(Color32::GRAY));
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
            4.0,
            Stroke::new(1.0, Color32::from_gray(85)),
        );
    }

    ui.add_space(4.0);

    resp.clicked()
}
