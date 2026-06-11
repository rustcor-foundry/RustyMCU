use bevy_egui::egui::{self, Color32, ProgressBar, RichText, ScrollArea, Ui};
use crate::plugins::flash::{DiskChannel, DiskCommand, FlashFile, FlashMode, FlashOp, FlashState, ProbeChannel, ProbeCommand};
use crate::state::LogKind;
use super::{export, theme};

const GREEN:  Color32 = theme::ACCENT;
const ORANGE: Color32 = theme::WARN;
const RED:    Color32 = theme::DANGER;
const GRAY:   Color32 = theme::TEXT3;

/// Stage → progress bar color, using the Fathom semantic palette.
fn op_color(op: &FlashOp) -> Color32 {
    match op {
        FlashOp::Erasing     => theme::WARN,
        FlashOp::Programming => theme::INFO,
        FlashOp::Verifying   => theme::ACCENT_DIM,
        FlashOp::Writing     => theme::INFO,
        FlashOp::Done        => theme::ACCENT,
        FlashOp::Error(_)    => theme::DANGER,
        FlashOp::Idle        => theme::TEXT3,
    }
}

pub fn draw(
    ui: &mut Ui,
    state: &mut FlashState,
    probe_ch: &ProbeChannel,
    disk_ch: &DiskChannel,
) {
    let busy = state.op.is_busy();

    // ── Log at bottom ─────────────────────────────────────────────────────────
    egui::TopBottomPanel::bottom("flash_log")
        .resizable(true)
        .min_height(80.0)
        .default_height(120.0)
        .show_inside(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Log").size(11.0).color(GRAY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("💾").on_hover_text("Export log (txt / csv / json)").clicked() {
                        export::log_dialog("flash_log.txt", &state.log.iter().collect::<Vec<_>>());
                    }
                });
            });
            ui.separator();
            let row_h = ui.text_style_height(&egui::TextStyle::Monospace);
            egui::Frame::none()
                .fill(theme::BG_DEEP)
                .rounding(egui::Rounding::same(4.0))
                .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                .show(ui, |ui| {
                    ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .stick_to_bottom(true)
                        .show_rows(ui, row_h, state.log.len(), |ui, range| {
                            for line in &state.log[range] {
                                let color = match line.kind {
                                    LogKind::Error  => RED,
                                    LogKind::Warn   => ORANGE,
                                    LogKind::System => theme::TEXT3,
                                    LogKind::Info   => theme::TEXT2,
                                };
                                ui.label(RichText::new(&line.text).monospace().size(11.0).color(color));
                            }
                        });
                });
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        // ── Mode toggle ───────────────────────────────────────────────────────
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.add_enabled_ui(!busy, |ui| {
                if ui.selectable_label(state.mode == FlashMode::Probe, "MCU Flash").clicked() {
                    state.mode = FlashMode::Probe;
                }
                if ui.selectable_label(state.mode == FlashMode::Disk, "SD / eMMC").clicked() {
                    state.mode = FlashMode::Disk;
                }
            });
        });
        ui.separator();
        ui.add_space(8.0);

        match state.mode {
            FlashMode::Probe => draw_probe_mode(ui, state, probe_ch, busy),
            FlashMode::Disk  => draw_disk_mode(ui, state, disk_ch, busy),
        }

        // ── Progress ──────────────────────────────────────────────────────────
        if state.op != FlashOp::Idle {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.set_min_width(ui.available_width() - 24.0);
                    let color = op_color(&state.op);

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(state.op.label()).size(12.0).color(color));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("{:.0}%", state.progress * 100.0))
                                    .size(12.0).monospace().color(color),
                            );
                        });
                    });
                    ui.add_space(4.0);
                    ui.add(
                        ProgressBar::new(state.progress)
                            .fill(color)
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.add_space(12.0);
            });
        }

        if matches!(state.op, FlashOp::Done) {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(RichText::new("● Complete").size(13.0).color(GREEN));
            });
        } else if matches!(state.op, FlashOp::Error(_)) {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(RichText::new("✕ Failed — see log").size(13.0).color(RED));
            });
        }
    });
}

fn draw_probe_mode(
    ui: &mut Ui,
    state: &mut FlashState,
    probe_ch: &ProbeChannel,
    busy: bool,
) {
    // Probe selection
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(RichText::new("Probe ").size(12.0).color(GRAY));
        ui.add_enabled_ui(!busy, |ui| {
            let label = state.probes.get(state.probe_idx)
                .map(|p| format!("{} ({})", p.identifier, p.vid_pid))
                .unwrap_or_else(|| "no probe found".into());
            egui::ComboBox::from_id_salt("probe_select")
                .selected_text(&label)
                .width(280.0)
                .show_ui(ui, |ui| {
                    for (i, probe) in state.probes.iter().enumerate() {
                        let text = format!("{} ({})", probe.identifier, probe.vid_pid);
                        ui.selectable_value(&mut state.probe_idx, i, text);
                    }
                });
            if ui.small_button("↺").clicked() {
                let _ = probe_ch.tx.send(ProbeCommand::ListProbes);
            }
        });
    });

    ui.add_space(6.0);

    // Target chip
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(RichText::new("Target").size(12.0).color(GRAY));
        ui.add_space(4.0);
        ui.add_enabled_ui(!busy, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.target)
                    .desired_width(200.0)
                    .hint_text("e.g. CH32V307VC, nRF52840, esp32c3"),
            );
        });
    });

    ui.add_space(6.0);

    // Firmware file
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(RichText::new("File   ").size(12.0).color(GRAY));
        ui.add_space(4.0);

        let name_text = state.probe_file.as_ref()
            .map(|f| f.name.clone())
            .unwrap_or_else(|| "no file selected".into());
        ui.add(
            egui::TextEdit::singleline(&mut name_text.clone())
                .desired_width(220.0)
                .interactive(false)
                .font(egui::TextStyle::Monospace)
                .text_color(if state.probe_file.is_some() { theme::TEXT } else { GRAY }),
        );

        ui.add_enabled_ui(!busy, |ui| {
            if ui.button("Browse…").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Select firmware")
                    .add_filter("Firmware", &["elf", "bin", "hex", "uf2"])
                    .add_filter("All files", &["*"])
                    .pick_file()
                {
                    let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                    let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                    let ext  = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    let is_elf = !matches!(ext.as_str(), "bin" | "hex" | "uf2");
                    state.probe_file = Some(FlashFile { path, name, size, is_elf });
                    state.op         = FlashOp::Idle;
                }
            }
        });

        if let Some(ref f) = state.probe_file {
            let tag = if f.is_elf { "ELF" } else { "bin" };
            ui.label(
                RichText::new(format!("{tag} · {}", fmt_size(f.size)))
                    .size(11.0).color(GRAY).monospace(),
            );
        }
    });

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    // Action buttons
    let has_file   = state.probe_file.is_some();
    let has_probe  = !state.probes.is_empty();
    let has_target = !state.target.is_empty();
    let can_flash  = has_file && has_probe && has_target && !busy;
    let can_erase  = has_probe && has_target && !busy;

    ui.horizontal(|ui| {
        ui.add_space(12.0);

        ui.add_enabled_ui(can_flash, |ui| {
            if theme::accent_button(ui, "⚡ Flash + Verify").clicked() {
                let f = state.probe_file.as_ref().unwrap();
                let _ = probe_ch.tx.send(ProbeCommand::Flash {
                    probe_idx: state.probe_idx,
                    target: state.target.clone(),
                    path: f.path.clone(),
                    verify: true,
                });
            }
        });

        ui.add_enabled_ui(can_flash, |ui| {
            if ui.button(RichText::new("↑ Flash").size(13.0)).clicked() {
                let f = state.probe_file.as_ref().unwrap();
                let _ = probe_ch.tx.send(ProbeCommand::Flash {
                    probe_idx: state.probe_idx,
                    target: state.target.clone(),
                    path: f.path.clone(),
                    verify: false,
                });
            }
        });

        ui.add_enabled_ui(can_erase, |ui| {
            if theme::danger_button(ui, "🗑 Erase").clicked() {
                let _ = probe_ch.tx.send(ProbeCommand::Erase {
                    probe_idx: state.probe_idx,
                    target: state.target.clone(),
                });
            }
        });

        if busy && theme::danger_button(ui, "✕ Cancel").clicked() {
            state.cancel();
        }
    });
}

fn draw_disk_mode(
    ui: &mut Ui,
    state: &mut FlashState,
    disk_ch: &DiskChannel,
    busy: bool,
) {
    // Drive selection
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(RichText::new("Drive ").size(12.0).color(GRAY));
        ui.add_enabled_ui(!busy, |ui| {
            let label = state.drives.get(state.drive_idx)
                .map(|d| d.display())
                .unwrap_or_else(|| "no drives found".into());
            egui::ComboBox::from_id_salt("drive_select")
                .selected_text(&label)
                .width(300.0)
                .show_ui(ui, |ui| {
                    for (i, drive) in state.drives.iter().enumerate() {
                        ui.selectable_value(&mut state.drive_idx, i, drive.display());
                    }
                });
            if ui.small_button("↺").clicked() {
                let _ = disk_ch.tx.send(DiskCommand::ListDrives);
            }
        });
    });

    ui.add_space(6.0);

    // Image file
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(RichText::new("Image ").size(12.0).color(GRAY));
        ui.add_space(4.0);

        let name_text = state.disk_file.as_ref()
            .map(|f| f.name.clone())
            .unwrap_or_else(|| "no file selected".into());
        ui.add(
            egui::TextEdit::singleline(&mut name_text.clone())
                .desired_width(220.0)
                .interactive(false)
                .font(egui::TextStyle::Monospace)
                .text_color(if state.disk_file.is_some() { theme::TEXT } else { GRAY }),
        );

        ui.add_enabled_ui(!busy, |ui| {
            if ui.button("Browse…").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Select disk image")
                    .add_filter("Disk images", &["img", "iso", "gz"])
                    .add_filter("All files", &["*"])
                    .pick_file()
                {
                    let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                    let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                    state.disk_file = Some(FlashFile { path, name, size, is_elf: false });
                    state.write_confirmed = false;
                }
            }
        });

        if let Some(ref f) = state.disk_file {
            let tag = if f.name.ends_with(".gz") { "gzip" }
                      else if f.name.ends_with(".iso") { "ISO" }
                      else { "raw" };
            ui.label(
                RichText::new(format!("{tag} · {}", fmt_size(f.size)))
                    .size(11.0).color(GRAY).monospace(),
            );
        }
    });

    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(
            RichText::new("⚠ This will OVERWRITE the selected drive. All data will be lost.")
                .size(11.0).color(ORANGE),
        );
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.add_enabled_ui(!busy, |ui| {
            ui.checkbox(&mut state.write_confirmed, "I understand — write the image");
        });
    });

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    let has_file  = state.disk_file.is_some();
    let has_drive = !state.drives.is_empty();
    let can_write = has_file && has_drive && state.write_confirmed && !busy;

    ui.horizontal(|ui| {
        ui.add_space(12.0);

        ui.add_enabled_ui(can_write, |ui| {
            if theme::accent_button(ui, "↓ Write Image").clicked() {
                let drive_path = state.drives[state.drive_idx].path.clone();
                let image_path = state.disk_file.as_ref().unwrap().path.clone();
                let _ = disk_ch.tx.send(DiskCommand::Write { drive_path, image_path });
            }
        });

        if busy && theme::danger_button(ui, "✕ Cancel").clicked() {
            state.cancel();
        }
    });
}

fn fmt_size(bytes: u64) -> String {
    if bytes >= 1 << 30      { format!("{:.1} GB", bytes as f64 / (1u64 << 30) as f64) }
    else if bytes >= 1 << 20 { format!("{:.0} MB", bytes as f64 / (1u64 << 20) as f64) }
    else if bytes >= 1 << 10 { format!("{:.0} KB", bytes as f64 / (1u64 << 10) as f64) }
    else                      { format!("{bytes} B") }
}
