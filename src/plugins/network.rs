use bevy::prelude::*;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::collections::VecDeque;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

use crate::state::{LogKind, LogLine};

// ── Channel types ─────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct TcpChannel {
    pub rx: Receiver<TcpEvent>,
    pub tx: mpsc::UnboundedSender<TcpCommand>,
}

pub enum TcpEvent {
    Connected { peer: String },
    Disconnected,
    Data(Vec<u8>),
    Error(String),
}

pub enum TcpCommand {
    Connect { host: String, port: u16 },
    Disconnect,
    Send(Vec<u8>),
}

// ── UI-facing state ───────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct NetworkState {
    pub host: String,
    pub port: String,
    pub connected: bool,
    pub log: VecDeque<LogLine>,
    pub rx_bytes: usize,
    pub tx_bytes: usize,
    pub input: String,
    pub paused: bool,
}

impl Default for NetworkState {
    fn default() -> Self {
        Self {
            host: "203.0.113.42".into(),
            port: "23".into(),
            connected: false,
            log: VecDeque::new(),
            rx_bytes: 0,
            tx_bytes: 0,
            input: String::new(),
            paused: false,
        }
    }
}

impl NetworkState {
    const MAX_LINES: usize = 4096;

    pub fn push(&mut self, line: LogLine) {
        if self.log.len() >= Self::MAX_LINES {
            self.log.pop_front();
        }
        self.log.push_back(line);
    }

    pub fn clear(&mut self) {
        self.log.clear();
        self.rx_bytes = 0;
        self.tx_bytes = 0;
    }
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        let (event_tx, event_rx) = bounded::<TcpEvent>(1024);
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<TcpCommand>();

        // Spawn a dedicated tokio runtime on a background OS thread.
        std::thread::Builder::new()
            .name("network-io".into())
            .spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("tokio runtime")
                    .block_on(tcp_worker(event_tx, cmd_rx));
            })
            .expect("spawn network-io thread");

        app.insert_resource(NetworkState::default())
            .insert_resource(TcpChannel { rx: event_rx, tx: cmd_tx })
            .add_systems(Update, poll_tcp_events);
    }
}

// ── Async TCP worker ──────────────────────────────────────────────────────────

async fn tcp_worker(tx: Sender<TcpEvent>, mut cmd_rx: mpsc::UnboundedReceiver<TcpCommand>) {
    'outer: loop {
        // Idle — wait for a Connect command.
        while let Some(cmd) = cmd_rx.recv().await {
            if let TcpCommand::Connect { host, port } = cmd {
                let addr = format!("{host}:{port}");
                match tokio::net::TcpStream::connect(&addr).await {
                    Ok(stream) => {
                        let peer = stream
                            .peer_addr()
                            .map(|a| a.to_string())
                            .unwrap_or(addr);
                        let _ = tx.send(TcpEvent::Connected { peer });
                        run_connected(stream, &tx, &mut cmd_rx).await;
                    }
                    Err(e) => {
                        let _ = tx.send(TcpEvent::Error(e.to_string()));
                    }
                }
                continue 'outer;
            } // Disconnect/Send while idle — ignore.
        }
        break; // Sender dropped — shut down.
    }
}

async fn run_connected(
    stream: tokio::net::TcpStream,
    tx: &Sender<TcpEvent>,
    cmd_rx: &mut mpsc::UnboundedReceiver<TcpCommand>,
) {
    let (mut reader, mut writer) = tokio::io::split(stream);
    let mut buf = [0u8; 1024];

    loop {
        tokio::select! {
            result = reader.read(&mut buf) => {
                match result {
                    Ok(0) => {
                        let _ = tx.send(TcpEvent::Disconnected);
                        return;
                    }
                    Ok(n) => {
                        let _ = tx.send(TcpEvent::Data(buf[..n].to_vec()));
                    }
                    Err(e) => {
                        let _ = tx.send(TcpEvent::Error(e.to_string()));
                        return;
                    }
                }
            }
            Some(cmd) = cmd_rx.recv() => {
                match cmd {
                    TcpCommand::Disconnect => {
                        let _ = tx.send(TcpEvent::Disconnected);
                        return;
                    }
                    TcpCommand::Send(data) => {
                        if let Err(e) = writer.write_all(&data).await {
                            let _ = tx.send(TcpEvent::Error(e.to_string()));
                            return;
                        }
                    }
                    TcpCommand::Connect { .. } => {} // Reconnect while connected — ignore.
                }
            }
        }
    }
}

// ── Bevy system: drain events into NetworkState ───────────────────────────────

fn poll_tcp_events(
    ch: Res<TcpChannel>,
    mut state: ResMut<NetworkState>,
    time: Res<Time>,
) {
    let ms = time.elapsed().as_millis() as u64;

    while let Ok(event) = ch.rx.try_recv() {
        if state.paused {
            continue;
        }
        match event {
            TcpEvent::Connected { peer } => {
                state.connected = true;
                state.push(LogLine {
                    timestamp_ms: ms,
                    text: format!("connected to {peer}"),
                    kind: LogKind::System,
                });
            }
            TcpEvent::Disconnected => {
                state.connected = false;
                state.push(LogLine {
                    timestamp_ms: ms,
                    text: "disconnected".into(),
                    kind: LogKind::System,
                });
            }
            TcpEvent::Data(bytes) => {
                state.rx_bytes += bytes.len();
                // Split on newlines; display each line. Non-UTF-8 bytes → hex escape.
                let text = bytes
                    .split(|&b| b == b'\n')
                    .map(|chunk| {
                        let s = String::from_utf8_lossy(chunk);
                        s.trim_end_matches('\r').to_string()
                    })
                    .filter(|s| !s.is_empty());
                for line in text {
                    state.push(LogLine { timestamp_ms: ms, text: line, kind: LogKind::Info });
                }
            }
            TcpEvent::Error(e) => {
                state.connected = false;
                state.push(LogLine {
                    timestamp_ms: ms,
                    text: format!("error: {e}"),
                    kind: LogKind::Error,
                });
            }
        }
    }
}
