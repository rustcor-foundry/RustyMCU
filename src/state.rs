use bevy::prelude::*;
use std::collections::VecDeque;

// ── Active tab ────────────────────────────────────────────────────────────────

#[derive(Resource, Default, PartialEq, Clone, Copy, Debug)]
pub enum ActiveTab {
    #[default]
    Serial,
    Plot,
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
            Box::new(
                self.lines
                    .iter()
                    .filter(move |l| l.text.to_lowercase().contains(&f)),
            )
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
    /// Previously sent commands, oldest first. Recalled with ↑/↓.
    pub history: Vec<String>,
    /// Cursor into `history` while browsing with the arrow keys; `None`
    /// means the user is typing a fresh line.
    pub history_idx: Option<usize>,
}

impl Default for SerialInput {
    fn default() -> Self {
        Self {
            text: String::new(),
            encoding: Encoding::Ascii,
            line_ending: LineEnding::Lf,
            history: Vec::new(),
            history_idx: None,
        }
    }
}

impl SerialInput {
    const MAX_HISTORY: usize = 64;

    pub fn push_history(&mut self, entry: String) {
        if self.history.last() != Some(&entry) {
            self.history.push(entry);
            if self.history.len() > Self::MAX_HISTORY {
                self.history.remove(0);
            }
        }
        self.history_idx = None;
    }

    /// ↑ — step back through history. Returns the recalled entry.
    pub fn history_prev(&mut self) -> Option<&str> {
        if self.history.is_empty() {
            return None;
        }
        let idx = match self.history_idx {
            None => self.history.len() - 1,
            Some(0) => 0,
            Some(i) => i - 1,
        };
        self.history_idx = Some(idx);
        self.history.get(idx).map(|s| s.as_str())
    }

    /// ↓ — step forward; past the newest entry returns an empty line.
    pub fn history_next(&mut self) -> Option<&str> {
        let idx = self.history_idx?;
        if idx + 1 >= self.history.len() {
            self.history_idx = None;
            Some("")
        } else {
            self.history_idx = Some(idx + 1);
            self.history.get(idx + 1).map(|s| s.as_str())
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
    Cr,
    CrLf,
    None,
}

impl LineEnding {
    pub const ALL: [Self; 4] = [Self::Lf, Self::Cr, Self::CrLf, Self::None];

    pub fn label(self) -> &'static str {
        match self {
            Self::Lf => "LF",
            Self::Cr => "CR",
            Self::CrLf => "CR+LF",
            Self::None => "none",
        }
    }

    pub fn bytes(self) -> &'static [u8] {
        match self {
            Self::Lf => b"\n",
            Self::Cr => b"\r",
            Self::CrLf => b"\r\n",
            Self::None => b"",
        }
    }
}

// ── Serial plotter ────────────────────────────────────────────────────────────
//
// Arduino-Serial-Plotter-style numeric stream capture. A serial line is
// treated as a data sample only when *every* whitespace/comma-separated token
// is numeric ("1.0 2.5") or labelled-numeric ("temp:23.4,hum:40") — mixed
// text lines are ignored so ordinary logs don't pollute the chart.

pub struct PlotSeries {
    pub name: String,
    pub points: VecDeque<[f64; 2]>, // [sample index, value]
}

#[derive(Resource)]
pub struct PlotState {
    pub series: Vec<PlotSeries>,
    pub sample_idx: u64,
    pub paused: bool,
    /// Number of most-recent samples shown (x-axis window).
    pub window: usize,
}

impl Default for PlotState {
    fn default() -> Self {
        Self {
            series: Vec::new(),
            sample_idx: 0,
            paused: false,
            window: 500,
        }
    }
}

impl PlotState {
    const MAX_POINTS: usize = 4096;

    pub fn clear(&mut self) {
        self.series.clear();
        self.sample_idx = 0;
    }

    /// Parse one serial line; record it if it is pure numeric data.
    pub fn ingest(&mut self, line: &str) {
        if self.paused {
            return;
        }
        let mut values: Vec<(Option<&str>, f64)> = Vec::new();
        for token in line.split([',', '\t', ' ']).filter(|t| !t.is_empty()) {
            let (label, num) = match token.split_once(':') {
                Some((l, n)) => (Some(l.trim()), n.trim()),
                None => (None, token),
            };
            match num.parse::<f64>() {
                Ok(v) if v.is_finite() => values.push((label, v)),
                _ => return, // non-numeric token → not a data line
            }
        }
        if values.is_empty() {
            return;
        }

        let x = self.sample_idx as f64;
        for (i, (label, v)) in values.into_iter().enumerate() {
            let name = label
                .map(str::to_owned)
                .unwrap_or_else(|| format!("S{}", i + 1));
            let series = match self.series.iter_mut().find(|s| s.name == name) {
                Some(s) => s,
                None => {
                    self.series.push(PlotSeries {
                        name,
                        points: VecDeque::new(),
                    });
                    self.series.last_mut().unwrap()
                }
            };
            if series.points.len() >= Self::MAX_POINTS {
                series.points.pop_front();
            }
            series.points.push_back([x, v]);
        }
        self.sample_idx += 1;
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
