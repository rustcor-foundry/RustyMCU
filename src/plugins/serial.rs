use bevy::prelude::*;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::io::Write;
use std::time::Duration;
use crate::plugins::defmt_decode::DefmtState;
use crate::state::{ConnectedDevices, Encoding, LinkStatus, LogKind, LogLine, SerialBuffer, SerialInput};

// ── Channel types ─────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct SerialChannel {
    pub rx: Receiver<SerialEvent>,
    pub tx: Sender<SerialCommand>,
}

/// Raw bytes from the device, plus lifecycle events.
/// Line-splitting and encoding-specific formatting happen in the Bevy system,
/// so the thread stays generic.
pub enum SerialEvent {
    Bytes(Vec<u8>),
    Connected(String),
    Disconnected,
    Error(String),
}

pub enum SerialCommand {
    Connect { port: String, baud: u32 },
    Disconnect,
    Send(Vec<u8>),
}

// ── Port scanner ──────────────────────────────────────────────────────────────

pub const COMMON_BAUDS: &[u32] = &[
    9_600, 19_200, 38_400, 57_600, 115_200, 230_400, 460_800, 921_600,
];

#[derive(Resource)]
pub struct PortScanner {
    pub ports: Vec<String>,
    pub selected: String,
    pub baud: u32,
    pub new_port_hint: Option<String>,
    timer: Timer,
    initialized: bool,
}

impl Default for PortScanner {
    fn default() -> Self {
        Self {
            ports: Vec::new(),
            selected: String::new(),
            baud: 115_200,
            new_port_hint: None,
            timer: Timer::from_seconds(2.0, TimerMode::Repeating),
            initialized: false,
        }
    }
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct SerialPlugin;

impl Plugin for SerialPlugin {
    fn build(&self, app: &mut App) {
        let (event_tx, event_rx) = bounded::<SerialEvent>(1024);
        let (cmd_tx, cmd_rx) = bounded::<SerialCommand>(64);

        std::thread::Builder::new()
            .name("serial-io".into())
            .spawn(move || serial_thread(event_tx, cmd_rx))
            .expect("spawn serial-io thread");

        app.insert_resource(SerialBuffer::default())
            .insert_resource(SerialInput::default())
            .insert_resource(PortScanner::default())
            .insert_resource(SerialChannel { rx: event_rx, tx: cmd_tx })
            .add_systems(Startup, initial_port_scan)
            .add_systems(Update, (poll_serial_events, periodic_port_scan));
    }
}

// ── Port scanning ─────────────────────────────────────────────────────────────

fn initial_port_scan(mut scanner: ResMut<PortScanner>) {
    refresh_ports(&mut scanner);
}

fn periodic_port_scan(mut scanner: ResMut<PortScanner>, time: Res<Time>) {
    scanner.timer.tick(time.delta());
    if scanner.timer.just_finished() {
        refresh_ports(&mut scanner);
    }
}

fn refresh_ports(scanner: &mut PortScanner) {
    let mut ports: Vec<String> = serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.port_name)
        .collect();
    ports.sort();

    if scanner.initialized {
        for port in &ports {
            if !scanner.ports.contains(port) && scanner.new_port_hint.is_none() {
                scanner.new_port_hint = Some(port.clone());
            }
        }
    }
    scanner.initialized = true;

    if !scanner.selected.is_empty() && ports.contains(&scanner.selected) {
        // keep
    } else {
        scanner.selected = ports.first().cloned().unwrap_or_default();
    }
    scanner.ports = ports;
}

// ── Background I/O thread ─────────────────────────────────────────────────────

fn serial_thread(tx: Sender<SerialEvent>, rx: Receiver<SerialCommand>) {
    let mut port: Option<Box<dyn serialport::SerialPort>> = None;
    let mut read_buf = [0u8; 512];

    loop {
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                SerialCommand::Connect { port: name, baud } => {
                    match serialport::new(&name, baud)
                        .timeout(Duration::from_millis(10))
                        .open()
                    {
                        Ok(p) => {
                            let _ = tx.send(SerialEvent::Connected(name));
                            port = Some(p);
                        }
                        Err(e) => {
                            let _ = tx.send(SerialEvent::Error(e.to_string()));
                        }
                    }
                }
                SerialCommand::Disconnect => {
                    port = None;
                    let _ = tx.send(SerialEvent::Disconnected);
                }
                SerialCommand::Send(bytes) => {
                    if let Some(p) = port.as_mut() {
                        let _ = p.write_all(&bytes);
                    }
                }
            }
        }

        if let Some(p) = port.as_mut() {
            match p.read(&mut read_buf) {
                Ok(n) if n > 0 => {
                    // Send raw bytes — all framing/encoding happens in the Bevy system.
                    let _ = tx.send(SerialEvent::Bytes(read_buf[..n].to_vec()));
                }
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) => {
                    port = None;
                    let _ = tx.send(SerialEvent::Error(e.to_string()));
                }
                _ => {}
            }
        } else {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

// ── Bevy system: drain channel, route per encoding ────────────────────────────

pub fn poll_serial_events(
    channel: Res<SerialChannel>,
    mut buf: ResMut<SerialBuffer>,
    mut devices: ResMut<ConnectedDevices>,
    input: Res<SerialInput>,
    mut defmt: ResMut<DefmtState>,
    time: Res<Time>,
    mut line_acc: Local<String>, // persists between frames; replaces in-thread accumulator
) {
    let ms = time.elapsed().as_millis() as u64;

    while let Ok(event) = channel.rx.try_recv() {
        match event {
            SerialEvent::Bytes(bytes) => {
                if buf.paused {
                    continue;
                }
                buf.rx_bytes += bytes.len();

                match input.encoding {
                    Encoding::Defmt => {
                        // Feed raw bytes through the defmt decoder.
                        for (text, kind) in defmt.feed(&bytes) {
                            buf.push(LogLine { timestamp_ms: ms, text, kind });
                        }
                    }
                    _ if buf.hex_view => {
                        // Hex dump: 16-byte lines.
                        for line in hex_dump_lines(&bytes) {
                            buf.push(LogLine {
                                timestamp_ms: ms,
                                text: line,
                                kind: LogKind::Info,
                            });
                        }
                    }
                    _ => {
                        // Text: accumulate into lines, split on \n.
                        for &b in &bytes {
                            match b {
                                b'\n' => {
                                    let text = std::mem::take(&mut *line_acc);
                                    if !text.is_empty() {
                                        let kind = classify(&text);
                                        buf.push(LogLine { timestamp_ms: ms, text, kind });
                                    }
                                }
                                b'\r' => {}
                                _ => line_acc.push(b as char),
                            }
                        }
                    }
                }
            }

            SerialEvent::Connected(port) => {
                if let Some(ref mut s) = devices.serial {
                    s.status = LinkStatus::Connected;
                    s.port = port.clone();
                }
                buf.push(LogLine {
                    timestamp_ms: ms,
                    text: format!("connected to {port}"),
                    kind: LogKind::System,
                });
            }
            SerialEvent::Disconnected => {
                if let Some(ref mut s) = devices.serial {
                    s.status = LinkStatus::Disconnected;
                }
                line_acc.clear();
                defmt.reset_decoder();
                buf.push(LogLine {
                    timestamp_ms: ms,
                    text: "disconnected".into(),
                    kind: LogKind::System,
                });
            }
            SerialEvent::Error(e) => {
                if let Some(ref mut s) = devices.serial {
                    s.status = LinkStatus::Error(e.clone());
                }
                buf.push(LogLine {
                    timestamp_ms: ms,
                    text: format!("error: {e}"),
                    kind: LogKind::Error,
                });
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

pub fn classify(text: &str) -> LogKind {
    if text.contains("WARN") || text.contains("warn") {
        LogKind::Warn
    } else if text.contains("ERROR") || text.contains("error") || text.contains("ERR:") {
        LogKind::Error
    } else {
        LogKind::Info
    }
}

/// Format raw bytes as a Wireshark-style hex dump, 16 bytes per line.
pub fn hex_dump_lines(bytes: &[u8]) -> Vec<String> {
    bytes
        .chunks(16)
        .enumerate()
        .map(|(i, chunk)| {
            let offset = i * 16;
            let hex: String = chunk
                .iter()
                .enumerate()
                .flat_map(|(j, b)| {
                    let sep = if j == 8 { "  " } else if j == 0 { "" } else { " " };
                    [sep.to_string(), format!("{b:02x}")]
                })
                .collect();
            let ascii: String = chunk
                .iter()
                .map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { '.' })
                .collect();
            // Pad hex column to fixed width (16 bytes × 3 chars + 1 extra space = 49)
            format!("{offset:04x}:  {hex:<49}  |{ascii}|")
        })
        .collect()
}
