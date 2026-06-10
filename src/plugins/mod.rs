use bevy::prelude::*;
use crate::state::{demo_devices, demo_serial_log, SerialBuffer};

pub mod defmt_decode;
pub mod flash;
pub mod network;
pub mod serial;
pub mod tray;
pub mod usb;

pub struct BoardPlugins;

impl Plugin for BoardPlugins {
    fn build(&self, app: &mut App) {
        app.insert_resource(demo_devices())
            .add_plugins(serial::SerialPlugin)
            .add_plugins(usb::UsbPlugin)
            .add_plugins(network::NetworkPlugin)
            .add_plugins(flash::FlashPlugin)
            .add_plugins(defmt_decode::DefmtPlugin)
            .add_plugins(tray::TrayPlugin)
            .add_systems(Startup, seed_demo_log);
    }
}

fn seed_demo_log(mut buf: ResMut<SerialBuffer>) {
    for line in demo_serial_log() {
        buf.push(line);
    }
    buf.rx_bytes = 1229;
    buf.tx_bytes = 84;
}
