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

