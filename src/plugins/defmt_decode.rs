use crate::plugins::flash::FlashState;
use crate::state::LogKind;
use bevy::prelude::*;
use std::path::PathBuf;

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct DefmtPlugin;

impl Plugin for DefmtPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DefmtState::default())
            .add_systems(Update, watch_flash_elf);
    }
}

// ── State ─────────────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct DefmtState {
    pub loaded_from: Option<PathBuf>,
    pub status: DefmtStatus,
    table: Option<defmt_decoder::Table>,
    pending: Vec<u8>,
}

#[derive(Default, Clone, Debug)]
pub enum DefmtStatus {
    #[default]
    NoElf,
    Loaded {
        path: String,
    },
    NoTable,
    Error(String),
}

impl DefmtStatus {
    pub fn label(&self) -> String {
        match self {
            Self::NoElf => "no ELF loaded".into(),
            Self::NoTable => "ELF has no defmt table".into(),
            Self::Loaded { path } => format!("✓ {path}"),
            Self::Error(e) => format!("error: {e}"),
        }
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Loaded { .. })
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

impl DefmtState {
    /// Feed raw serial bytes through the decoder.  Returns `(text, kind)` pairs
    /// for every complete frame decoded.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<(String, LogKind)> {
        let Some(ref table) = self.table else {
            return vec![(
                "[defmt] no ELF table loaded — switch encoding to ASCII".into(),
                LogKind::System,
            )];
        };

        self.pending.extend_from_slice(bytes);

        let mut out = Vec::new();
        loop {
            match table.decode(&self.pending) {
                Ok((frame, consumed)) => {
                    let kind = match frame.level() {
                        Some(defmt_parser::Level::Error) => LogKind::Error,
                        Some(defmt_parser::Level::Warn) => LogKind::Warn,
                        _ => LogKind::Info,
                    };
                    let text = frame.display(false).to_string();
                    self.pending.drain(..consumed);
                    out.push((text, kind));
                }
                Err(defmt_decoder::DecodeError::UnexpectedEof) => break,
                Err(defmt_decoder::DecodeError::Malformed) => {
                    if !self.pending.is_empty() {
                        self.pending.drain(..1);
                    } else {
                        break;
                    }
                }
            }
        }
        out
    }

    fn load(&mut self, path: &PathBuf) {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.status = DefmtStatus::Error(e.to_string());
                self.table = None;
                return;
            }
        };

        match defmt_decoder::Table::parse(&bytes) {
            Ok(Some(table)) => {
                let label = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                self.status = DefmtStatus::Loaded { path: label };
                self.table = Some(table);
                self.pending.clear();
                self.loaded_from = Some(path.clone());
            }
            Ok(None) => {
                self.status = DefmtStatus::NoTable;
                self.table = None;
            }
            Err(e) => {
                self.status = DefmtStatus::Error(e.to_string());
                self.table = None;
            }
        }
    }

    /// Discard any buffered bytes (useful when reconnecting serial).
    pub fn reset_decoder(&mut self) {
        self.pending.clear();
    }
}

// ── System: reload table when Flash panel loads a new ELF ────────────────────

fn watch_flash_elf(flash: Res<FlashState>, mut defmt: ResMut<DefmtState>) {
    let Some(ref file) = flash.probe_file else {
        return;
    };
    let already_loaded = defmt.loaded_from.as_ref() == Some(&file.path);
    if !already_loaded {
        defmt.load(&file.path);
    }
}
