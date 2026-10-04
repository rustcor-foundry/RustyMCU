use crate::plugins::usb::UsbDeviceInfo;
use crate::state::{LogKind, LogLine};
use std::io::{BufWriter, Write};
use std::path::Path;

// ── Log export ────────────────────────────────────────────────────────────────

/// Open a save-file dialog and write `lines` in whichever format the user picks.
pub fn log_dialog(name_hint: &str, lines: &[&LogLine]) {
    let Some(path) = rfd::FileDialog::new()
        .set_title("Export log")
        .add_filter("Text log", &["txt", "log"])
        .add_filter("CSV", &["csv"])
        .add_filter("JSON", &["json"])
        .set_file_name(name_hint)
        .save_file()
    else {
        return;
    };

    if let Err(e) = write_log(&path, lines) {
        eprintln!("export error: {e}");
    }
}

fn write_log(path: &Path, lines: &[&LogLine]) -> std::io::Result<()> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "csv" => write_log_csv(path, lines),
        "json" => write_log_json(path, lines),
        _ => write_log_txt(path, lines),
    }
}

fn write_log_txt(path: &Path, lines: &[&LogLine]) -> std::io::Result<()> {
    let mut w = BufWriter::new(std::fs::File::create(path)?);
    for l in lines {
        writeln!(
            w,
            "{} [{:<6}] {}",
            fmt_ts(l.timestamp_ms),
            kind_str(&l.kind),
            l.text
        )?;
    }
    Ok(())
}

fn write_log_csv(path: &Path, lines: &[&LogLine]) -> std::io::Result<()> {
    let mut w = BufWriter::new(std::fs::File::create(path)?);
    writeln!(w, "timestamp_ms,level,message")?;
    for l in lines {
        writeln!(
            w,
            "{},{},{}",
            l.timestamp_ms,
            kind_str(&l.kind),
            csv_esc(&l.text)
        )?;
    }
    Ok(())
}

fn write_log_json(path: &Path, lines: &[&LogLine]) -> std::io::Result<()> {
    let mut w = BufWriter::new(std::fs::File::create(path)?);
    writeln!(w, "[")?;
    let last = lines.len().saturating_sub(1);
    for (i, l) in lines.iter().enumerate() {
        let tail = if i < last { "," } else { "" };
        writeln!(
            w,
            "  {{\"timestamp_ms\":{},\"level\":{},\"message\":{}}}{}",
            l.timestamp_ms,
            json_str(kind_str(&l.kind)),
            json_str(&l.text),
            tail,
        )?;
    }
    writeln!(w, "]")?;
    Ok(())
}

// ── USB export ────────────────────────────────────────────────────────────────

/// Open a save-file dialog and write `devices` in whichever format the user picks.
pub fn usb_dialog(devices: &[UsbDeviceInfo]) {
    let Some(path) = rfd::FileDialog::new()
        .set_title("Export USB device list")
        .add_filter("Text", &["txt"])
        .add_filter("CSV", &["csv"])
        .add_filter("JSON", &["json"])
        .set_file_name("usb_devices.csv")
        .save_file()
    else {
        return;
    };

    if let Err(e) = write_usb(&path, devices) {
        eprintln!("export error: {e}");
    }
}

fn write_usb(path: &Path, devices: &[UsbDeviceInfo]) -> std::io::Result<()> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "csv" => write_usb_csv(path, devices),
        "json" => write_usb_json(path, devices),
        _ => write_usb_txt(path, devices),
    }
}

fn write_usb_txt(path: &Path, devices: &[UsbDeviceInfo]) -> std::io::Result<()> {
    let mut w = BufWriter::new(std::fs::File::create(path)?);
    writeln!(
        w,
        "{:<12}  {:<24}  {:<32}  {:<20}  {:<6}  Speed",
        "VID:PID", "Manufacturer", "Product", "Serial", "Class"
    )?;
    writeln!(
        w,
        "{:-<12}  {:-<24}  {:-<32}  {:-<20}  {:-<6}  {:-<10}",
        "", "", "", "", "", ""
    )?;
    for d in devices {
        writeln!(
            w,
            "{:<12}  {:<24}  {:<32}  {:<20}  {:#06x}  {}",
            d.vid_pid(),
            pad_to(&d.manufacturer, 24),
            pad_to(&d.product, 32),
            pad_to(&d.serial, 20),
            d.class,
            d.speed,
        )?;
    }
    Ok(())
}

fn write_usb_csv(path: &Path, devices: &[UsbDeviceInfo]) -> std::io::Result<()> {
    let mut w = BufWriter::new(std::fs::File::create(path)?);
    writeln!(w, "vid,pid,manufacturer,product,serial,class,speed")?;
    for d in devices {
        writeln!(
            w,
            "{:04x},{:04x},{},{},{},{:#04x},{}",
            d.vendor_id,
            d.product_id,
            csv_esc(&d.manufacturer),
            csv_esc(&d.product),
            csv_esc(&d.serial),
            d.class,
            csv_esc(&d.speed),
        )?;
    }
    Ok(())
}

fn write_usb_json(path: &Path, devices: &[UsbDeviceInfo]) -> std::io::Result<()> {
    let mut w = BufWriter::new(std::fs::File::create(path)?);
    writeln!(w, "[")?;
    let last = devices.len().saturating_sub(1);
    for (i, d) in devices.iter().enumerate() {
        let tail = if i < last { "," } else { "" };
        writeln!(
            w,
            "  {{\"vid\":{},\"pid\":{},\"manufacturer\":{},\"product\":{},\"serial\":{},\"class\":{},\"speed\":{}}}{}",
            json_str(&format!("{:04x}", d.vendor_id)),
            json_str(&format!("{:04x}", d.product_id)),
            json_str(&d.manufacturer),
            json_str(&d.product),
            json_str(&d.serial),
            d.class,
            json_str(&d.speed),
            tail,
        )?;
    }
    writeln!(w, "]")?;
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn kind_str(kind: &LogKind) -> &'static str {
    match kind {
        LogKind::System => "SYSTEM",
        LogKind::Info => "INFO",
        LogKind::Warn => "WARN",
        LogKind::Error => "ERROR",
    }
}

fn fmt_ts(ms: u64) -> String {
    let mins = ms / 60_000;
    let secs = (ms % 60_000) / 1_000;
    let millis = ms % 1_000;
    format!("[{mins:02}:{secs:02}.{millis:03}]")
}

fn csv_esc(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn pad_to(s: &str, max_chars: usize) -> String {
    let mut out: String = s.chars().take(max_chars).collect();
    while out.len() < max_chars {
        out.push(' ');
    }
    out
}
