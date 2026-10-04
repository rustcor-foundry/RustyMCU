//! Fathom "deep ocean" design tokens, ported from theme.css.
//! Abyssal blue-black surfaces, phosphor-green accent, teal-tinted text.

use bevy_egui::egui::{self, Color32, Rounding, Stroke};

// ── Accent — phosphor green (sonar ping) ─────────────────────────────────────
pub const ACCENT:     Color32 = Color32::from_rgb(0x00, 0xe8, 0x7a);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(0x00, 0xb8, 0x5e);

// ── Semantic ──────────────────────────────────────────────────────────────────
pub const DANGER: Color32 = Color32::from_rgb(0xff, 0x4d, 0x6d);
pub const WARN:   Color32 = Color32::from_rgb(0xf5, 0xc8, 0x42);
pub const INFO:   Color32 = Color32::from_rgb(0x00, 0xc8, 0xd4);

// ── Deep ocean surfaces ───────────────────────────────────────────────────────
pub const BG_DEEP: Color32 = Color32::from_rgb(0x04, 0x08, 0x10); // terminal wells
pub const BG:      Color32 = Color32::from_rgb(0x06, 0x0b, 0x12); // app background
pub const BG2:     Color32 = Color32::from_rgb(0x0d, 0x1f, 0x30); // raised panels
pub const BG3:     Color32 = Color32::from_rgb(0x12, 0x25, 0x38); // cards
pub const BG4:     Color32 = Color32::from_rgb(0x18, 0x30, 0x48); // buttons

pub const BORDER:       Color32 = Color32::from_rgb(0x27, 0x4d, 0x65);
pub const BORDER_LIGHT: Color32 = Color32::from_rgb(0x1e, 0x3a, 0x52);

// ── Text ──────────────────────────────────────────────────────────────────────
pub const TEXT:  Color32 = Color32::from_rgb(0xd8, 0xf2, 0xe8);
pub const TEXT2: Color32 = Color32::from_rgb(0xa9, 0xd3, 0xc7);
pub const TEXT3: Color32 = Color32::from_rgb(0x7f, 0xb3, 0xa6);

const ROUND: Rounding = Rounding::same(6.0);

/// Apply the theme to the egui context. Idempotent; called every frame.
pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let v = &mut style.visuals;

    v.dark_mode = true;
    v.panel_fill = BG;
    v.window_fill = BG2;
    v.extreme_bg_color = BG_DEEP; // text edits, scroll wells
    v.faint_bg_color = BG2;
    v.code_bg_color = BG_DEEP;
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(0x00, 0xe8, 0x7a, 50);
    v.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    v.window_rounding = ROUND;
    v.menu_rounding = ROUND;

    // Labels, separators
    v.widgets.noninteractive.bg_fill = BG2;
    v.widgets.noninteractive.weak_bg_fill = BG2;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER_LIGHT);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT2);
    v.widgets.noninteractive.rounding = ROUND;

    // Buttons, combo boxes at rest
    v.widgets.inactive.bg_fill = BG4;
    v.widgets.inactive.weak_bg_fill = BG3;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    v.widgets.inactive.rounding = ROUND;

    v.widgets.hovered.bg_fill = BG3;
    v.widgets.hovered.weak_bg_fill = BG4;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, TEXT3);
    v.widgets.hovered.fg_stroke = Stroke::new(1.5_f32, TEXT);
    v.widgets.hovered.rounding = ROUND;

    v.widgets.active.bg_fill = BG4;
    v.widgets.active.weak_bg_fill = BG4;
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    v.widgets.active.fg_stroke = Stroke::new(1.5_f32, TEXT);
    v.widgets.active.rounding = ROUND;

    v.widgets.open.bg_fill = BG3;
    v.widgets.open.weak_bg_fill = BG3;
    v.widgets.open.bg_stroke = Stroke::new(1.0_f32, BORDER);
    v.widgets.open.fg_stroke = Stroke::new(1.0_f32, TEXT);
    v.widgets.open.rounding = ROUND;

    style.spacing.item_spacing = egui::vec2(8.0, 5.0);
    style.spacing.button_padding = egui::vec2(12.0, 5.0);

    ctx.set_style(style);
}

/// Primary action button: accent fill, dark text — Fathom's `.btn.accent`.
pub fn accent_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let fill = if ui.is_enabled() { ACCENT_DIM } else { BG4 };
    let text = egui::RichText::new(label).size(13.0).strong().color(BG_DEEP);
    ui.add(egui::Button::new(text).fill(fill).stroke(Stroke::new(1.0_f32, ACCENT)))
}

/// Destructive action button: transparent fill, danger outline — `.btn.danger`.
pub fn danger_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let text = egui::RichText::new(label).size(13.0).color(DANGER);
    ui.add(
        egui::Button::new(text)
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0_f32, DANGER)),
    )
}
