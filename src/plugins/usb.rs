use bevy::prelude::*;
use crate::state::{ConnectedDevices, DebugProbeInfo, LinkStatus, UsbHsInfo};

// ── Known WCH / CH32 USB identifiers ─────────────────────────────────────────

const WCH_VID: u16 = 0x1a86;

/// WCH-LinkE debug probe (various firmware versions).
const WCHLINK_PIDS: &[u16] = &[
    0x8008, // WCH-Link (original)
    0x8010, // WCH-LinkE
    0x8011, // WCH-LinkE (alt)
    0x8012, // WCH-LinkE CDC
];

/// CH32 USB HS CDC-ACM (firmware-side USB, not the link probe).
const CH32_CDC_PIDS: &[u16] = &[
    0x5523, // CH32V CDC
    0x5512, // CH32V CDC (alt)
    0x7523, // CH32X CDC
    0x55d3, // CH32V307 CDC-ACM
];

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct UsbDeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: String,
    pub product: String,
    pub serial: String,
    pub speed: String,
    pub class: u8,
}

impl UsbDeviceInfo {
    pub fn is_wch(&self) -> bool {
        self.vendor_id == WCH_VID
    }

    pub fn vid_pid(&self) -> String {
        format!("{:04x}:{:04x}", self.vendor_id, self.product_id)
    }

    fn is_link_probe(&self) -> bool {
        self.vendor_id == WCH_VID && WCHLINK_PIDS.contains(&self.product_id)
    }

    fn is_ch32_cdc(&self) -> bool {
        self.vendor_id == WCH_VID && CH32_CDC_PIDS.contains(&self.product_id)
    }
}

#[derive(Resource, Default)]
pub struct UsbDeviceList {
    pub devices: Vec<UsbDeviceInfo>,
    pub error: Option<String>,
    timer: Option<Timer>,
}

impl UsbDeviceList {
    fn start_timer(&mut self) {
        self.timer = Some(Timer::from_seconds(3.0, TimerMode::Repeating));
    }
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct UsbPlugin;

impl Plugin for UsbPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(UsbDeviceList::default())
            .add_systems(Startup, initial_usb_scan)
            .add_systems(Update, periodic_usb_scan);
    }
}

fn initial_usb_scan(
    mut list: ResMut<UsbDeviceList>,
    mut devices: ResMut<ConnectedDevices>,
) {
    list.start_timer();
    scan(&mut list);
    apply_to_connected(&list, &mut devices);
}

fn periodic_usb_scan(
    mut list: ResMut<UsbDeviceList>,
    mut devices: ResMut<ConnectedDevices>,
    time: Res<Time>,
) {
    let finished = list
        .timer
        .as_mut()
        .map(|t| { t.tick(time.delta()); t.just_finished() })
        .unwrap_or(false);

    if finished {
        scan(&mut list);
        apply_to_connected(&list, &mut devices);
    }
}

// ── USB enumeration ───────────────────────────────────────────────────────────

fn scan(list: &mut UsbDeviceList) {
    match nusb::list_devices() {
        Ok(iter) => {
            list.error = None;
            list.devices = iter
                .map(|d| UsbDeviceInfo {
                    vendor_id: d.vendor_id(),
                    product_id: d.product_id(),
                    manufacturer: d.manufacturer_string().unwrap_or("").to_string(),
                    product: d.product_string().unwrap_or("").to_string(),
                    serial: d.serial_number().unwrap_or("").to_string(),
                    speed: format!("{:?}", d.speed()),
                    class: d.class(),
                })
                .collect();
        }
        Err(e) => {
            list.error = Some(e.to_string());
        }
    }
}

// ── Update ConnectedDevices from current USB device list ──────────────────────

fn apply_to_connected(list: &UsbDeviceList, devices: &mut ConnectedDevices) {
    // ── WCH-LinkE debug probe ─────────────────────────────────────────────────
    let probe_dev = list.devices.iter().find(|d| d.is_link_probe());
    match probe_dev {
        Some(d) => {
            let name = if d.product.is_empty() { "WCH-LinkE".to_string() } else { d.product.clone() };
            match &mut devices.debug_probe {
                Some(p) => {
                    p.name = name;
                    p.status = LinkStatus::Connected;
                }
                None => {
                    devices.debug_probe = Some(DebugProbeInfo {
                        name,
                        transport: "USB HID · debug".into(),
                        status: LinkStatus::Connected,
                    });
                }
            }
        }
        None => {
            if let Some(ref mut p) = devices.debug_probe {
                p.status = LinkStatus::Disconnected;
            }
        }
    }

    // ── CH32 CDC-ACM (USB HS firmware side) ───────────────────────────────────
    let cdc_dev = list.devices.iter().find(|d| d.is_ch32_cdc());
    match cdc_dev {
        Some(d) => {
            let product = if d.product.is_empty() { "USB HS".to_string() } else { d.product.clone() };
            match &mut devices.usb_hs {
                Some(u) => {
                    u.class = product;
                    u.note = format!("CDC-ACM · {}", d.vid_pid());
                    u.status = LinkStatus::Connected;
                }
                None => {
                    devices.usb_hs = Some(UsbHsInfo {
                        class: product,
                        note: format!("CDC-ACM · {}", d.vid_pid()),
                        status: LinkStatus::Connected,
                    });
                }
            }
        }
        None => {
            if let Some(ref mut u) = devices.usb_hs {
                u.status = LinkStatus::Disconnected;
                u.note = "CDC-ACM · idle".into();
            }
        }
    }
}
