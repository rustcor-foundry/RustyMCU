use bevy_egui::egui::{self, Color32, Grid, RichText, ScrollArea, Ui};
use crate::plugins::usb::UsbDeviceList;
use super::{export, theme};

const WCH_COLOR: Color32 = theme::ACCENT;

pub fn draw(ui: &mut Ui, list: &UsbDeviceList) {
    // ── Error banner ──────────────────────────────────────────────────────────
    if let Some(err) = &list.error {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(
                RichText::new(format!("USB enumeration error: {err}"))
                    .color(theme::DANGER)
                    .size(12.0),
            );
        });
        return;
    }

    // ── Header ────────────────────────────────────────────────────────────────
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(RichText::new("USB Devices").size(13.0).strong());
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!("({} found)", list.devices.len()))
                .size(12.0)
                .color(theme::TEXT3),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(12.0);
            if ui.small_button("💾 Export").on_hover_text("Export device list (txt / csv / json)").clicked() {
                export::usb_dialog(&list.devices);
            }
        });
    });
    ui.add_space(8.0);
    ui.separator();

    if list.devices.is_empty() {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(RichText::new("No USB devices detected").color(theme::TEXT3));
        });
        return;
    }

    // ── Device table ──────────────────────────────────────────────────────────
    ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        ui.add_space(4.0);

        // Column headers
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            Grid::new("usb_header")
                .num_columns(6)
                .spacing([16.0, 0.0])
                .show(ui, |ui| {
                    for label in ["VID:PID", "Manufacturer", "Product", "Serial", "Class", "Speed"] {
                        ui.label(
                            RichText::new(label)
                                .size(10.0)
                                .color(theme::TEXT3)
                                .monospace(),
                        );
                    }
                    ui.end_row();
                });
        });

        ui.separator();

        // Rows
        for (idx, dev) in list.devices.iter().enumerate() {
            let highlight = dev.is_wch();
            let text_color = if highlight { WCH_COLOR } else { theme::TEXT2 };

            egui::Frame::none()
                .inner_margin(egui::Margin { left: 12.0, right: 12.0, top: 3.0, bottom: 3.0 })
                .show(ui, |ui| {
                    Grid::new(format!("usb_row_{idx}"))
                        .num_columns(6)
                        .spacing([16.0, 0.0])
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(dev.vid_pid())
                                    .monospace()
                                    .size(12.0)
                                    .color(text_color),
                            );
                            ui.label(
                                RichText::new(if dev.manufacturer.is_empty() { "—" } else { &dev.manufacturer })
                                    .monospace()
                                    .size(12.0)
                                    .color(text_color),
                            );
                            ui.label(
                                RichText::new(if dev.product.is_empty() { "—" } else { &dev.product })
                                    .monospace()
                                    .size(12.0)
                                    .color(text_color),
                            );
                            ui.label(
                                RichText::new(if dev.serial.is_empty() { "—" } else { &dev.serial })
                                    .monospace()
                                    .size(12.0)
                                    .color(theme::TEXT3),
                            );
                            ui.label(
                                RichText::new(format!("{:#04x}", dev.class))
                                    .monospace()
                                    .size(12.0)
                                    .color(theme::TEXT3),
                            );
                            ui.label(
                                RichText::new(&dev.speed)
                                    .monospace()
                                    .size(12.0)
                                    .color(theme::TEXT3),
                            );
                            ui.end_row();
                        });
                });

            // WCH device hint
            if highlight {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("  ↳ WCH device — CH32 / WCH-Link family")
                            .size(11.0)
                            .color(WCH_COLOR),
                    );
                });
            }
        }

        ui.add_space(8.0);
    });
}
