use bevy::prelude::*;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::io::{Read, Write};
use std::path::PathBuf;
use crate::state::{LogKind, LogLine};

// ── Public info types ─────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ProbeInfo {
    pub identifier: String,
    pub vid_pid: String,
}

#[derive(Clone, Debug)]
pub struct DriveInfo {
    pub path: String,
    pub model: String,
    pub size_bytes: u64,
    pub removable: bool,
}

impl DriveInfo {
    pub fn display(&self) -> String {
        let tag = if self.removable { " [removable]" } else { "" };
        format!("{}{} — {}", self.model, tag, self.size_label())
    }
    pub fn size_label(&self) -> String {
        let gb = self.size_bytes as f64 / 1_073_741_824.0;
        if gb >= 1.0 { format!("{:.1} GB", gb) }
        else { format!("{:.0} MB", self.size_bytes as f64 / 1_048_576.0) }
    }
}

// ── Flash state ───────────────────────────────────────────────────────────────

#[derive(Default, Clone, Copy, PartialEq)]
pub enum FlashMode { #[default] Probe, Disk }

#[derive(Default, Clone, PartialEq, Debug)]
pub enum FlashOp {
    #[default] Idle,
    Erasing,
    Programming,
    Verifying,
    Writing,
    Done,
    Error(String),
}

impl FlashOp {
    pub fn is_busy(&self) -> bool {
        matches!(self, FlashOp::Erasing | FlashOp::Programming | FlashOp::Verifying | FlashOp::Writing)
    }
    pub fn label(&self) -> &str {
        match self {
            FlashOp::Idle        => "idle",
            FlashOp::Erasing     => "erasing…",
            FlashOp::Programming => "programming…",
            FlashOp::Verifying   => "verifying…",
            FlashOp::Writing     => "writing…",
            FlashOp::Done        => "done ✓",
            FlashOp::Error(_)    => "error",
        }
    }
    pub fn progress_color(&self) -> bevy_egui::egui::Color32 {
        use bevy_egui::egui::Color32;
        match self {
            FlashOp::Erasing     => Color32::from_rgb(200, 140, 50),
            FlashOp::Programming => Color32::from_rgb(80, 140, 220),
            FlashOp::Verifying   => Color32::from_rgb(80, 200, 120),
            FlashOp::Writing     => Color32::from_rgb(80, 140, 220),
            FlashOp::Done        => Color32::from_rgb(99, 180, 50),
            FlashOp::Error(_)    => Color32::from_rgb(200, 80, 80),
            FlashOp::Idle        => Color32::GRAY,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FlashFile {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub is_elf: bool,
}

#[derive(Resource, Default)]
pub struct FlashState {
    pub op: FlashOp,
    pub progress: f32,
    pub log: Vec<LogLine>,
    pub mode: FlashMode,

    // probe mode
    pub probe_file: Option<FlashFile>,
    pub probes: Vec<ProbeInfo>,
    pub probe_idx: usize,
    pub target: String,

    // disk mode
    pub disk_file: Option<FlashFile>,
    pub drives: Vec<DriveInfo>,
    pub drive_idx: usize,
    pub write_confirmed: bool,
}

impl FlashState {
    pub fn cancel(&mut self) {
        self.op = FlashOp::Idle;
        self.progress = 0.0;
        self.write_confirmed = false;
    }
}

// ── Channels ──────────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct ProbeChannel {
    pub rx: Receiver<ProbeEvent>,
    pub tx: Sender<ProbeCommand>,
}

#[derive(Resource)]
pub struct DiskChannel {
    pub rx: Receiver<DiskEvent>,
    pub tx: Sender<DiskCommand>,
}

pub enum ProbeCommand {
    ListProbes,
    Flash { probe_idx: usize, target: String, path: PathBuf, verify: bool },
    Erase { probe_idx: usize, target: String },
}

pub enum DiskCommand {
    ListDrives,
    Write { drive_path: String, image_path: PathBuf },
}

pub enum ProbeEvent {
    ProbeList(Vec<ProbeInfo>),
    Progress { op: FlashOp, percent: f32 },
    Log(String, LogKind),
    Done,
    Error(String),
}

pub enum DiskEvent {
    DriveList(Vec<DriveInfo>),
    Progress { written: u64, total: u64 },
    Log(String, LogKind),
    Done,
    Error(String),
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct FlashPlugin;

impl Plugin for FlashPlugin {
    fn build(&self, app: &mut App) {
        let (probe_evt_tx, probe_evt_rx) = bounded::<ProbeEvent>(256);
        let (probe_cmd_tx, probe_cmd_rx) = bounded::<ProbeCommand>(16);
        let (disk_evt_tx,  disk_evt_rx)  = bounded::<DiskEvent>(256);
        let (disk_cmd_tx,  disk_cmd_rx)  = bounded::<DiskCommand>(16);

        std::thread::Builder::new()
            .name("probe-flash".into())
            .spawn(move || probe_thread(probe_evt_tx, probe_cmd_rx))
            .expect("spawn probe-flash");

        std::thread::Builder::new()
            .name("disk-flash".into())
            .spawn(move || disk_thread(disk_evt_tx, disk_cmd_rx))
            .expect("spawn disk-flash");

        app.insert_resource(FlashState::default())
            .insert_resource(ProbeChannel { rx: probe_evt_rx, tx: probe_cmd_tx })
            .insert_resource(DiskChannel  { rx: disk_evt_rx,  tx: disk_cmd_tx  })
            .add_systems(Startup, startup_scan)
            .add_systems(Update, (poll_probe_events, poll_disk_events));
    }
}

fn startup_scan(probe_ch: Res<ProbeChannel>, disk_ch: Res<DiskChannel>) {
    let _ = probe_ch.tx.send(ProbeCommand::ListProbes);
    let _ = disk_ch.tx.send(DiskCommand::ListDrives);
}

// ── Bevy poll systems ─────────────────────────────────────────────────────────

fn poll_probe_events(ch: Res<ProbeChannel>, mut state: ResMut<FlashState>, time: Res<Time>) {
    let ms = time.elapsed().as_millis() as u64;
    while let Ok(event) = ch.rx.try_recv() {
        match event {
            ProbeEvent::ProbeList(list) => {
                if state.probe_idx >= list.len().max(1) { state.probe_idx = 0; }
                state.probes = list;
            }
            ProbeEvent::Progress { op, percent } => {
                state.op = op;
                state.progress = percent;
            }
            ProbeEvent::Log(text, kind) => {
                state.log.push(LogLine { timestamp_ms: ms, text, kind });
            }
            ProbeEvent::Done => {
                state.op = FlashOp::Done;
                state.progress = 1.0;
            }
            ProbeEvent::Error(e) => {
                let msg = e.clone();
                state.op = FlashOp::Error(e);
                state.log.push(LogLine { timestamp_ms: ms, text: format!("✕ {msg}"), kind: LogKind::Error });
            }
        }
    }
}

fn poll_disk_events(ch: Res<DiskChannel>, mut state: ResMut<FlashState>, time: Res<Time>) {
    let ms = time.elapsed().as_millis() as u64;
    while let Ok(event) = ch.rx.try_recv() {
        match event {
            DiskEvent::DriveList(list) => {
                if state.drive_idx >= list.len().max(1) { state.drive_idx = 0; }
                state.drives = list;
            }
            DiskEvent::Progress { written, total } => {
                state.op = FlashOp::Writing;
                state.progress = if total > 0 { written as f32 / total as f32 } else { 0.0 };
            }
            DiskEvent::Log(text, kind) => {
                state.log.push(LogLine { timestamp_ms: ms, text, kind });
            }
            DiskEvent::Done => {
                state.op = FlashOp::Done;
                state.progress = 1.0;
                state.write_confirmed = false;
            }
            DiskEvent::Error(e) => {
                let msg = e.clone();
                state.op = FlashOp::Error(e);
                state.log.push(LogLine { timestamp_ms: ms, text: format!("✕ {msg}"), kind: LogKind::Error });
                state.write_confirmed = false;
            }
        }
    }
}

// ── Probe thread ──────────────────────────────────────────────────────────────

fn probe_thread(tx: Sender<ProbeEvent>, rx: Receiver<ProbeCommand>) {
    loop {
        match rx.recv() {
            Err(_) => break,
            Ok(cmd) => match cmd {
                ProbeCommand::ListProbes => {
                    let _ = tx.send(ProbeEvent::ProbeList(scan_probes()));
                }
                ProbeCommand::Flash { probe_idx, target, path, verify } => {
                    do_probe_flash(&tx, probe_idx, &target, &path, verify);
                }
                ProbeCommand::Erase { probe_idx, target } => {
                    do_probe_erase(&tx, probe_idx, &target);
                }
            }
        }
    }
}

fn scan_probes() -> Vec<ProbeInfo> {
    probe_rs::probe::list::Lister::new()
        .list_all()
        .into_iter()
        .map(|info| ProbeInfo {
            identifier: info.identifier.clone(),
            vid_pid: format!("{:04x}:{:04x}", info.vendor_id, info.product_id),
        })
        .collect()
}

fn open_session(tx: &Sender<ProbeEvent>, probe_idx: usize, target: &str)
    -> Option<probe_rs::Session>
{
    let probes = probe_rs::probe::list::Lister::new().list_all();
    if probe_idx >= probes.len() {
        let _ = tx.send(ProbeEvent::Error(format!("probe #{probe_idx} not found")));
        return None;
    }
    let probe = match probes[probe_idx].open() {
        Ok(p) => p,
        Err(e) => { let _ = tx.send(ProbeEvent::Error(e.to_string())); return None; }
    };
    let _ = tx.send(ProbeEvent::Log(format!("attaching to {target}…"), LogKind::System));
    match probe.attach(target, probe_rs::Permissions::default()) {
        Ok(s) => Some(s),
        Err(e) => { let _ = tx.send(ProbeEvent::Error(e.to_string())); None }
    }
}

fn make_progress(tx: Sender<ProbeEvent>) -> probe_rs::flashing::FlashProgress<'static> {
    use probe_rs::flashing::{FlashProgress, ProgressEvent, ProgressOperation};

    let mut erase_total: u64 = 0;
    let mut erase_done:  u64 = 0;
    let mut prog_total:  u64 = 0;
    let mut prog_done:   u64 = 0;
    let mut verify_total: u64 = 0;
    let mut verify_done:  u64 = 0;

    FlashProgress::new(move |event| {
        match event {
            ProgressEvent::AddProgressBar { operation, total: Some(t) } => {
                match operation {
                    ProgressOperation::Erase   => erase_total  += t,
                    ProgressOperation::Program => prog_total   += t,
                    ProgressOperation::Verify  => verify_total += t,
                    _ => {}
                }
            }
            ProgressEvent::AddProgressBar { total: None, .. } => {}
            ProgressEvent::Started(op) => {
                let (stage, msg): (FlashOp, &str) = match op {
                    ProgressOperation::Erase   => (FlashOp::Erasing,     "erasing flash…"),
                    ProgressOperation::Program => (FlashOp::Programming,  "programming…"),
                    ProgressOperation::Verify  => (FlashOp::Verifying,    "verifying…"),
                    ProgressOperation::Fill    => (FlashOp::Erasing,      "reading for fill…"),
                };
                let _ = tx.send(ProbeEvent::Progress { op: stage, percent: 0.0 });
                let _ = tx.send(ProbeEvent::Log(msg.into(), LogKind::System));
            }
            ProgressEvent::Progress { operation, size, .. } => {
                match operation {
                    ProgressOperation::Erase => {
                        erase_done += size;
                        let pct = if erase_total > 0 { (erase_done as f32 / erase_total as f32).min(1.0) } else { 0.0 };
                        let _ = tx.send(ProbeEvent::Progress { op: FlashOp::Erasing, percent: pct });
                    }
                    ProgressOperation::Program => {
                        prog_done += size;
                        let pct = if prog_total > 0 { (prog_done as f32 / prog_total as f32).min(1.0) } else { 0.0 };
                        let _ = tx.send(ProbeEvent::Progress { op: FlashOp::Programming, percent: pct });
                    }
                    ProgressOperation::Verify => {
                        verify_done += size;
                        let pct = if verify_total > 0 { (verify_done as f32 / verify_total as f32).min(1.0) } else { 0.0 };
                        let _ = tx.send(ProbeEvent::Progress { op: FlashOp::Verifying, percent: pct });
                    }
                    _ => {}
                }
            }
            ProgressEvent::Finished(op) => {
                let msg = match op {
                    ProgressOperation::Erase   => "erase done",
                    ProgressOperation::Program => "programming done",
                    ProgressOperation::Verify  => "verify done",
                    ProgressOperation::Fill    => "fill done",
                };
                let _ = tx.send(ProbeEvent::Log(msg.into(), LogKind::System));
            }
            ProgressEvent::Failed(op) => {
                let msg = match op {
                    ProgressOperation::Erase   => "erase failed",
                    ProgressOperation::Program => "programming failed",
                    ProgressOperation::Verify  => "verify failed",
                    _ => "operation failed",
                };
                let _ = tx.send(ProbeEvent::Error(msg.into()));
            }
            ProgressEvent::DiagnosticMessage { message } => {
                let _ = tx.send(ProbeEvent::Log(message, LogKind::Info));
            }
            _ => {}
        }
    })
}

fn format_for_path(path: &std::path::Path) -> probe_rs::flashing::Format {
    use probe_rs::flashing::{BinOptions, ElfOptions, Format};
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "hex" | "ihex" => Format::Hex,
        "bin"          => Format::Bin(BinOptions::default()),
        "uf2"          => Format::Uf2,
        _              => Format::Elf(ElfOptions::default()),
    }
}

fn do_probe_flash(
    tx: &Sender<ProbeEvent>,
    probe_idx: usize,
    target: &str,
    path: &PathBuf,
    verify: bool,
) {
    use probe_rs::flashing::{download_file_with_options, DownloadOptions};

    let Some(mut session) = open_session(tx, probe_idx, target) else { return };
    let fname = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let _ = tx.send(ProbeEvent::Log(format!("flashing {fname}…"), LogKind::System));

    let mut options = DownloadOptions::default();
    options.progress      = make_progress(tx.clone());
    options.verify        = verify;
    options.do_chip_erase = true;

    match download_file_with_options(&mut session, path, format_for_path(path), options) {
        Ok(()) => {
            let _ = tx.send(ProbeEvent::Log("resetting target…".into(), LogKind::System));
            if let Ok(mut core) = session.core(0) {
                let _ = core.reset();
            }
            let _ = tx.send(ProbeEvent::Done);
        }
        Err(e) => { let _ = tx.send(ProbeEvent::Error(e.to_string())); }
    }
}

fn do_probe_erase(tx: &Sender<ProbeEvent>, probe_idx: usize, target: &str) {
    use probe_rs::flashing::erase_all;

    let Some(mut session) = open_session(tx, probe_idx, target) else { return };
    let _ = tx.send(ProbeEvent::Log("mass-erasing chip…".into(), LogKind::System));
    let _ = tx.send(ProbeEvent::Progress { op: FlashOp::Erasing, percent: 0.0 });

    let mut progress = make_progress(tx.clone());
    match erase_all(&mut session, &mut progress, false) {
        Ok(()) => { let _ = tx.send(ProbeEvent::Done); }
        Err(e) => { let _ = tx.send(ProbeEvent::Error(e.to_string())); }
    }
}

// ── Disk thread ───────────────────────────────────────────────────────────────

fn disk_thread(tx: Sender<DiskEvent>, rx: Receiver<DiskCommand>) {
    loop {
        match rx.recv() {
            Err(_) => break,
            Ok(DiskCommand::ListDrives) => {
                let _ = tx.send(DiskEvent::DriveList(enumerate_drives()));
            }
            Ok(DiskCommand::Write { drive_path, image_path }) => {
                do_write_image(&tx, &drive_path, &image_path);
            }
        }
    }
}

fn do_write_image(tx: &Sender<DiskEvent>, drive_path: &str, image_path: &PathBuf) {
    let fname = image_path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let is_gz = fname.ends_with(".gz");

    let _ = tx.send(DiskEvent::Log(format!("opening {fname}…"), LogKind::System));
    let src = match std::fs::File::open(image_path) {
        Ok(f) => f,
        Err(e) => { let _ = tx.send(DiskEvent::Error(e.to_string())); return; }
    };

    let _ = tx.send(DiskEvent::Log(format!("opening {drive_path}…"), LogKind::System));
    let mut dst = match open_drive_write(drive_path) {
        Ok(f) => f,
        Err(e) => {
            let _ = tx.send(DiskEvent::Error(
                format!("cannot open drive: {e} — try running as administrator")
            ));
            return;
        }
    };

    const CHUNK: usize = 1024 * 1024;
    let mut buf = vec![0u8; CHUNK];
    let mut written: u64 = 0;

    if is_gz {
        let gz = flate2::read::GzDecoder::new(std::io::BufReader::new(src));
        let mut reader = std::io::BufReader::with_capacity(CHUNK, gz);
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if let Err(e) = dst.write_all(&buf[..n]) {
                        let _ = tx.send(DiskEvent::Error(e.to_string())); return;
                    }
                    written += n as u64;
                    let _ = tx.send(DiskEvent::Progress { written, total: 0 });
                }
                Err(e) => { let _ = tx.send(DiskEvent::Error(e.to_string())); return; }
            }
        }
    } else {
        let total = image_path.metadata().map(|m| m.len()).unwrap_or(0);
        let mut reader = std::io::BufReader::with_capacity(CHUNK, src);
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if let Err(e) = dst.write_all(&buf[..n]) {
                        let _ = tx.send(DiskEvent::Error(e.to_string())); return;
                    }
                    written += n as u64;
                    let _ = tx.send(DiskEvent::Progress { written, total });
                }
                Err(e) => { let _ = tx.send(DiskEvent::Error(e.to_string())); return; }
            }
        }
    }

    if let Err(e) = dst.flush() {
        let _ = tx.send(DiskEvent::Error(e.to_string())); return;
    }
    let _ = tx.send(DiskEvent::Log(format!("wrote {}", fmt_bytes(written as usize)), LogKind::System));
    let _ = tx.send(DiskEvent::Done);
}

fn open_drive_write(path: &str) -> std::io::Result<std::fs::File> {
    #[cfg(windows)] {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true).write(true)
            .custom_flags(0x20000000u32) // FILE_FLAG_WRITE_THROUGH
            .open(path)
    }
    #[cfg(not(windows))] {
        std::fs::OpenOptions::new().write(true).open(path)
    }
}

fn enumerate_drives() -> Vec<DriveInfo> {
    #[cfg(windows)]      { windows_disk::list() }
    #[cfg(not(windows))] { posix_disk::list()   }
}

fn fmt_bytes(n: usize) -> String {
    if n >= 1 << 30      { format!("{:.2} GB", n as f64 / (1u64 << 30) as f64) }
    else if n >= 1 << 20 { format!("{:.1} MB", n as f64 / (1u64 << 20) as f64) }
    else if n >= 1 << 10 { format!("{:.1} KB", n as f64 / (1u64 << 10) as f64) }
    else                 { format!("{n} B") }
}

// ── Windows disk enumeration ──────────────────────────────────────────────────

#[cfg(windows)]
mod windows_disk {
    use super::DriveInfo;

    pub fn list() -> Vec<DriveInfo> {
        let result = std::process::Command::new("powershell")
            .args([
                "-NoProfile", "-NonInteractive", "-Command",
                r#"Get-WmiObject Win32_DiskDrive | ForEach-Object { "$($_.DeviceID)|$($_.Model)|$($_.Size)|$($_.MediaType)" }"#,
            ])
            .output();

        match result {
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stdout);
                let mut drives: Vec<DriveInfo> = text.lines().filter_map(parse_line).collect();
                if drives.is_empty() { drives = fallback(); }
                drives
            }
            Err(_) => fallback(),
        }
    }

    fn parse_line(line: &str) -> Option<DriveInfo> {
        let line = line.trim();
        if line.is_empty() { return None; }
        let p: Vec<&str> = line.splitn(4, '|').collect();
        if p.len() < 3 { return None; }
        let path  = p[0].trim().to_string();
        let model = p[1].trim().to_string();
        let size: u64 = p[2].trim().parse().ok()?;
        let media = p.get(3).unwrap_or(&"").to_lowercase();
        let removable = media.contains("removable") || media.contains("external");
        if path.is_empty() || size == 0 { return None; }
        Some(DriveInfo { path, model, size_bytes: size, removable })
    }

    fn fallback() -> Vec<DriveInfo> {
        use std::io::Seek;
        (0..16u32).filter_map(|i| {
            let path = format!("\\\\.\\PhysicalDrive{i}");
            let mut f = std::fs::File::open(&path).ok()?;
            let size = f.seek(std::io::SeekFrom::End(0)).ok()?;
            if size == 0 { return None; }
            Some(DriveInfo { path, model: format!("Disk {i}"), size_bytes: size, removable: false })
        }).collect()
    }
}

// ── POSIX disk enumeration ────────────────────────────────────────────────────

#[cfg(not(windows))]
mod posix_disk {
    use super::DriveInfo;

    pub fn list() -> Vec<DriveInfo> {
        let Ok(out) = std::process::Command::new("lsblk")
            .args(["-dno", "NAME,MODEL,SIZE,RM", "--bytes"])
            .output()
        else { return vec![] };

        String::from_utf8_lossy(&out.stdout).lines().filter_map(|line| {
            let p: Vec<&str> = line.split_whitespace().collect();
            if p.len() < 3 { return None; }
            let size: u64 = p[2].parse().ok()?;
            let removable = p.get(3).map(|r| *r == "1").unwrap_or(false);
            Some(DriveInfo {
                path: format!("/dev/{}", p[0]),
                model: p[1].to_string(),
                size_bytes: size,
                removable,
            })
        }).collect()
    }
}
