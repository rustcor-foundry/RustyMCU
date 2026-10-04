use crate::plugins::defmt_decode::DefmtState;
use crate::plugins::flash::FlashState;
use crate::plugins::flash::{DiskChannel, ProbeChannel};
use crate::plugins::network::{NetworkState, TcpChannel};
use crate::plugins::serial::{PortScanner, ReconnectState, SerialChannel};
use crate::plugins::usb::UsbDeviceList;
use crate::state::{ActiveTab, ConnectedDevices, PlotState, SerialBuffer, SerialInput};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

mod export;
mod flash_panel;
mod network_panel;
mod plot_panel;
mod serial_panel;
mod sidebar;
pub mod theme;
mod usb_panel;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ActiveTab::default())
            .add_systems(Update, draw_ui);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_ui(
    mut ctx: EguiContexts,
    mut active_tab: ResMut<ActiveTab>,
    devices: Res<ConnectedDevices>,
    mut serial_buf: ResMut<SerialBuffer>,
    mut serial_input: ResMut<SerialInput>,
    serial_ch: Res<SerialChannel>,
    mut port_scanner: ResMut<PortScanner>,
    mut reconnect: ResMut<ReconnectState>,
    mut plot_state: ResMut<PlotState>,
    usb_devices: Res<UsbDeviceList>,
    mut net_state: ResMut<NetworkState>,
    net_ch: Res<TcpChannel>,
    mut flash_state: ResMut<FlashState>,
    probe_ch: Res<ProbeChannel>,
    disk_ch: Res<DiskChannel>,
    defmt_state: Res<DefmtState>,
) {
    let ctx = ctx.ctx_mut();
    theme::apply(ctx);

    egui::SidePanel::left("sidebar")
        .exact_width(210.0)
        .resizable(false)
        .frame(
            egui::Frame::none()
                .fill(theme::BG2)
                .stroke(egui::Stroke::new(1.0_f32, theme::BORDER_LIGHT)),
        )
        .show(ctx, |ui| {
            sidebar::draw(
                ui,
                &devices,
                &mut active_tab,
                &mut port_scanner,
                &mut net_state,
            );
        });

    egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| tab_bar(ui, &mut active_tab));

    egui::CentralPanel::default().show(ctx, |ui| match *active_tab {
        ActiveTab::Serial => serial_panel::draw(
            ui,
            &mut serial_buf,
            &mut serial_input,
            &serial_ch,
            &mut port_scanner,
            &devices,
            &defmt_state,
            &mut reconnect,
        ),
        ActiveTab::Plot => plot_panel::draw(ui, &mut plot_state),
        ActiveTab::Usb => usb_panel::draw(ui, &usb_devices),
        ActiveTab::Network => network_panel::draw(ui, &mut net_state, &net_ch),
        ActiveTab::Flash => flash_panel::draw(ui, &mut flash_state, &probe_ch, &disk_ch),
    });
}

fn tab_bar(ui: &mut egui::Ui, active: &mut ActiveTab) {
    ui.horizontal(|ui| {
        ui.set_min_height(36.0);
        ui.add_space(8.0);
        tab_button(ui, active, ActiveTab::Serial, "⌨ Serial");
        tab_button(ui, active, ActiveTab::Plot, "📈 Plot");
        tab_button(ui, active, ActiveTab::Usb, "⎇ USB");
        tab_button(ui, active, ActiveTab::Network, "⊞ Network");
        tab_button(ui, active, ActiveTab::Flash, "↑ Flash");
    });
}

fn tab_button(ui: &mut egui::Ui, active: &mut ActiveTab, tab: ActiveTab, label: &str) {
    let is_active = *active == tab;
    let color = if is_active { theme::TEXT } else { theme::TEXT3 };
    let resp =
        ui.add(egui::Button::new(egui::RichText::new(label).size(13.0).color(color)).frame(false));
    if resp.clicked() {
        *active = tab;
    }
    if is_active {
        let y = resp.rect.max.y + 3.0;
        ui.painter().line_segment(
            [
                egui::pos2(resp.rect.min.x, y),
                egui::pos2(resp.rect.max.x, y),
            ],
            egui::Stroke::new(2.0_f32, theme::ACCENT),
        );
    }
}
