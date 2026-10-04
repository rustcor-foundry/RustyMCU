use crate::state::ConnectedDevices;
use bevy::prelude::*;

pub mod defmt_decode;
pub mod flash;
pub mod network;
pub mod serial;
pub mod tray;
pub mod usb;

pub struct BoardPlugins;

impl Plugin for BoardPlugins {
    fn build(&self, app: &mut App) {
        app.insert_resource(ConnectedDevices::default())
            .add_plugins(serial::SerialPlugin)
            .add_plugins(usb::UsbPlugin)
            .add_plugins(network::NetworkPlugin)
            .add_plugins(flash::FlashPlugin)
            .add_plugins(defmt_decode::DefmtPlugin)
            .add_plugins(tray::TrayPlugin);
    }
}
