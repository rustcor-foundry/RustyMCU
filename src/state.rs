use bevy::prelude::*;
use std::collections::VecDeque;

// ── Active tab ────────────────────────────────────────────────────────────────

#[derive(Resource, Default, PartialEq, Clone, Copy, Debug)]
pub enum ActiveTab {
    #[default]
    Serial,
    Usb,
    Network,
    Flash,
}

// ── Serial log buffer ─────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct LogLine {
    #[allow(dead_code)]
    pub timestamp_ms: u64,
    pub text: String,
    pub kind: LogKind,
}

#[derive(Clone, PartialEq, Debug)]
pub enum LogKind {
    System,
    Info,
    Warn,
    Error,
}

#[derive(Resource, Default)]
pub struct SerialBuffer {
    pub lines: VecDeque<LogLine>,
    pub rx_bytes: usize,
    pub tx_bytes: usize,
    pub paused: bool,
    pub show_timestamps: bool,
    pub filter: String,
    pub hex_view: bool,
}

impl SerialBuffer {
    const MAX_LINES: usize = 4096;

    pub fn push(&mut self, line: LogLine) {
        if self.lines.len() >= Self::MAX_LINES {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    /// Lines visible after applying the current filter.
    pub fn filtered<'a>(&'a self) -> Box<dyn Iterator<Item = &'a LogLine> + 'a> {
        if self.filter.is_empty() {
            Box::new(self.lines.iter())
        } else {
            let f = self.filter.to_lowercase();
            Box::new(self.lines.iter().filter(move |l| l.text.to_lowercase().contains(&f)))
        }
    }

    #[allow(dead_code)]
    pub fn filtered_count(&self) -> usize {
        self.filtered().count()
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.rx_bytes = 0;
        self.tx_bytes = 0;
    }
}

// ── Serial send bar state ─────────────────────────────────────────────────────

#[derive(Resource)]
pub struct SerialInput {
    pub text: String,
    pub encoding: Encoding,
    pub line_ending: LineEnding,
}

impl Default for SerialInput {
    fn default() -> Self {
        Self {
            text: String::new(),
            encoding: Encoding::Ascii,
            line_ending: LineEnding::Lf,
        }
    }
}

#[derive(PartialEq, Clone, Copy, Default)]
pub enum Encoding {
    #[default]
    Ascii,
    Hex,
    Defmt,
}

impl Encoding {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ascii => "ASCII",
            Self::Hex => "Hex",
            Self::Defmt => "defmt",
        }
    }
}

#[derive(PartialEq, Clone, Copy, Default)]
pub enum LineEnding {
    #[default]
    Lf,
    CrLf,
    None,
}

impl LineEnding {
    pub fn label(self) -> &'static str {
        match self {
            Self::Lf => "LF",
            Self::CrLf => "CR+LF",
            Self::None => "none",
        }
    }

    pub fn bytes(self) -> &'static [u8] {
        match self {
            Self::Lf => b"\n",
            Self::CrLf => b"\r\n",
            Self::None => b"",
        }
    }
}

// ── Connected devices ─────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Debug)]
pub enum LinkStatus {
    Connected,
    #[allow(dead_code)]
    Connecting,
    Disconnected,
    Error(String),
}

impl LinkStatus {
    pub fn is_up(&self) -> bool {
        matches!(self, Self::Connected)
    }
}

#[derive(Resource, Default)]
pub struct ConnectedDevices {
    pub debug_probe: Option<DebugProbeInfo>,
    pub serial: Option<SerialDevInfo>,
    pub ethernet: Option<EthernetInfo>,
    pub usb_hs: Option<UsbHsInfo>,
    pub chip: Option<ChipInfo>,
}

#[derive(Clone)]
pub struct DebugProbeInfo {
    pub name: String,
    pub transport: String,
    pub status: LinkStatus,
}

#[derive(Clone)]
pub struct SerialDevInfo {
    pub port: String,
    pub baud: u32,
    pub framing: String,
    pub status: LinkStatus,
}

#[derive(Clone)]
pub struct EthernetInfo {
    pub ip: String,
    pub speed: String,
    pub status: LinkStatus,
}

#[derive(Clone)]
pub struct UsbHsInfo {
    pub class: String,
    pub note: String,
    pub status: LinkStatus,
}

#[derive(Clone)]
pub struct ChipInfo {
    pub part: String,
    pub core: String,
    pub flash_kb: u32,
    pub ram_kb: u32,
    pub fw_version: String,
}

// ── Demo fixture ──────────────────────────────────────────────────────────────

pub fn demo_devices() -> ConnectedDevices {
    ConnectedDevices {
        debug_probe: Some(DebugProbeInfo {
            name: "WCH-LinkE".into(),
            transport: "USB HID · debug".into(),
            status: LinkStatus::Connected,
        }),
        serial: Some(SerialDevInfo {
            port: "COM7".into(),
            baud: 115200,
            framing: "8N1".into(),
            status: LinkStatus::Connected,
        }),
        ethernet: Some(EthernetInfo {
            ip: "203.0.113.42".into(),
            speed: "Ethernet · 100M".into(),
            status: LinkStatus::Connected,
        }),
        usb_hs: Some(UsbHsInfo {
            class: "USB HS".into(),
            note: "CDC-ACM · idle".into(),
            status: LinkStatus::Disconnected,
        }),
        chip: Some(ChipInfo {
            part: "CH32V307VCT6".into(),
            core: "RV32IMAFC · 144MHz".into(),
            flash_kb: 256,
            ram_kb: 64,
            fw_version: "v0.3.1-alpha".into(),
        }),
    }
}

pub fn demo_serial_log() -> Vec<LogLine> {
    vec![
        LogLine { timestamp_ms: 0,    text: "opening COM7 @ 115200 8N1...".into(), kind: LogKind::System },
        LogLine { timestamp_ms: 1,    text: "[00:00.001] board init OK".into(),             kind: LogKind::Info },
        LogLine { timestamp_ms: 12,   text: "[00:00.012] embassy executor started".into(),  kind: LogKind::Info },
        LogLine { timestamp_ms: 15,   text: "[00:00.015] usb cdc-acm ready".into(),         kind: LogKind::Info },
        LogLine { timestamp_ms: 20,   text: "[00:00.020] defmt tick=0".into(),              kind: LogKind::Info },
        LogLine { timestamp_ms: 25,   text: "[00:00.025] led task spawned".into(),          kind: LogKind::Info },
        LogLine { timestamp_ms: 1000, text: "[00:01.000] defmt tick=1000".into(),           kind: LogKind::Info },
        LogLine { timestamp_ms: 2000, text: "[00:02.000] defmt tick=2000".into(),           kind: LogKind::Info },
        LogLine { timestamp_ms: 3142, text: "[00:03.142] WARN: eth link down — retrying".into(), kind: LogKind::Warn },
        LogLine { timestamp_ms: 3450, text: "[00:03.450] eth link up · 100Mbps full-duplex".into(), kind: LogKind::Info },
        LogLine { timestamp_ms: 3451, text: "[00:03.451] dhcp lease → 203.0.113.42".into(), kind: LogKind::Info },
        LogLine { timestamp_ms: 4000, text: "[00:04.000] defmt tick=4000".into(),           kind: LogKind::Info },
    ]
}
